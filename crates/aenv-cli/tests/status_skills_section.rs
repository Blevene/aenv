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

#[test]
fn status_lists_authored_skill() {
    let h = Harness::new();
    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "my-skill"
mode = "authored"
adapter = "claude-code"
"#,
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/base/CLAUDE.md"), "hi").unwrap();
    std::fs::create_dir_all(h.aenv_home().join("envs/base/.claude/skills/my-skill")).unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/.claude/skills/my-skill/SKILL.md"),
        "---\nname: my-skill\ndescription: y\n---\nbody\n",
    )
    .unwrap();

    let out = h.cmd().args(["use", "base", "--project"]).arg(h.project()).output().unwrap();
    assert!(out.status.success(), "use failed: stderr={}", String::from_utf8_lossy(&out.stderr));

    let out = h.cmd().args(["activate", "--project"]).arg(h.project()).output().unwrap();
    assert!(out.status.success(), "activate failed: stderr={}", String::from_utf8_lossy(&out.stderr));

    let out = h.cmd().args(["status", "--project"]).arg(h.project()).output().unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Skills"), "stdout missing Skills section: {stdout}");
    assert!(stdout.contains("my-skill"), "stdout missing skill name: {stdout}");
    assert!(stdout.contains("authored"), "stdout missing 'authored' mode: {stdout}");
}
