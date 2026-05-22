//! End-to-end integration test reproducing spec §5.9–§5.11.
//! Uses the raw `std::process::Command` + `Harness` pattern (no assert_cmd).

use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: std::path::PathBuf,
    project: std::path::PathBuf,
}

impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        let project = std::fs::canonicalize(project_guard.path()).unwrap();
        Self {
            _aenv_home_guard: aenv_home_guard,
            _project_guard: project_guard,
            aenv_home,
            project,
        }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
        c.env("AENV_HOME", &self.aenv_home);
        c
    }

    fn aenv_home(&self) -> &Path {
        &self.aenv_home
    }

    fn project(&self) -> &Path {
        &self.project
    }
}

fn assert_success(label: &str, out: &std::process::Output) {
    assert!(
        out.status.success(),
        "{label} failed: status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn full_skills_lifecycle_matches_spec() {
    let h = Harness::new();

    // Spec §5.9: author a skill.
    assert_success(
        "create",
        &h.cmd()
            .args(["create", "detailed-execution"])
            .output()
            .unwrap(),
    );
    std::fs::write(
        h.aenv_home().join("envs/detailed-execution/aenv.toml"),
        "name = \"detailed-execution\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(
        h.aenv_home().join("envs/detailed-execution/CLAUDE.md"),
        "hi",
    )
    .unwrap();

    let out = h
        .cmd()
        .args([
            "skill",
            "new",
            "run-migration",
            "--ns",
            "detailed-execution",
        ])
        .output()
        .unwrap();
    assert_success("skill new", &out);
    let skill_new_stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        skill_new_stdout.contains("Created authored skill"),
        "expected 'Created authored skill' in stdout; got: {skill_new_stdout:?}"
    );
    assert!(
        h.aenv_home()
            .join("envs/detailed-execution/.claude/skills/run-migration/SKILL.md")
            .exists(),
        "SKILL.md not scaffolded in namespace"
    );

    // Spec §5.10: import a local skill.
    // Use a named subdirectory inside a tempdir so the derived skill name is
    // predictable ("check-before-submit") rather than a random tmpdir suffix.
    let external_root = tempdir().unwrap();
    let external_skill_dir = external_root.path().join("check-before-submit");
    std::fs::create_dir_all(&external_skill_dir).unwrap();
    std::fs::write(
        external_skill_dir.join("SKILL.md"),
        "---\nname: check-before-submit\ndescription: y\n---\n",
    )
    .unwrap();
    let canonical = std::fs::canonicalize(&external_skill_dir).unwrap();
    let out = h
        .cmd()
        .args(["skill", "import"])
        .arg(canonical.to_str().unwrap())
        .args(["--ns", "detailed-execution", "--adapter", "claude-code"])
        .output()
        .unwrap();
    assert_success("skill import local", &out);

    // Spec §5.11: list skills — both appear with correct modes.
    let out = h.cmd().args(["skill", "list"]).output().unwrap();
    assert_success("skill list", &out);
    let list_stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        list_stdout.contains("run-migration"),
        "expected 'run-migration' in skill list; got: {list_stdout:?}"
    );
    assert!(
        list_stdout.contains("check-before-submit"),
        "expected 'check-before-submit' in skill list; got: {list_stdout:?}"
    );
    assert!(
        list_stdout.contains("authored"),
        "expected 'authored' in skill list; got: {list_stdout:?}"
    );
    assert!(
        list_stdout.contains("imported"),
        "expected 'imported' in skill list; got: {list_stdout:?}"
    );

    // Activation works end-to-end — use --project flag (matches parameters_policies_e2e.rs pattern).
    assert_success(
        "use",
        &h.cmd()
            .args(["use", "detailed-execution", "--project"])
            .arg(h.project())
            .output()
            .unwrap(),
    );
    assert_success(
        "activate",
        &h.cmd()
            .args(["activate", "--project"])
            .arg(h.project())
            .output()
            .unwrap(),
    );

    // Authored skill materializes.
    assert!(
        h.project()
            .join(".claude/skills/run-migration/SKILL.md")
            .exists(),
        "authored skill SKILL.md not materialized"
    );

    // Imported skill materializes at .claude/skills/<derived-name>/SKILL.md
    // where derived-name == last component of the canonical source path
    // (i.e. "check-before-submit").
    let imported_path = h
        .project()
        .join(".claude/skills/check-before-submit/SKILL.md");
    assert!(
        imported_path.exists(),
        "expected imported skill at {} (project={})",
        imported_path.display(),
        h.project().display()
    );

    // Status shows Skills section containing run-migration.
    let out = h
        .cmd()
        .args(["status", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success("status", &out);
    let status_stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        status_stdout.contains("Skills"),
        "expected 'Skills' in status output; got: {status_stdout:?}"
    );
    assert!(
        status_stdout.contains("run-migration"),
        "expected 'run-migration' in status output; got: {status_stdout:?}"
    );
}
