use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

#[allow(dead_code)]
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
    #[allow(dead_code)]
    fn project(&self) -> &Path {
        &self.project
    }
}

#[test]
fn skill_new_scaffolds_skill_md_and_appends_manifest() {
    let h = Harness::new();
    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        "name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/base/CLAUDE.md"), "hi").unwrap();

    let out = h
        .cmd()
        .args(["skill", "new", "my-skill", "--ns", "base"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let skill_md = h.aenv_home().join("envs/base/.claude/skills/my-skill/SKILL.md");
    assert!(skill_md.exists(), "SKILL.md not created");
    let body = std::fs::read_to_string(&skill_md).unwrap();
    assert!(body.contains("name: my-skill"));
    assert!(body.contains("description:"));

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(manifest.contains("[[skills]]"));
    assert!(manifest.contains("name = \"my-skill\""));
    assert!(manifest.contains("mode = \"authored\""));
}

#[test]
fn skill_new_errors_when_namespace_missing() {
    let h = Harness::new();
    let out = h
        .cmd()
        .args(["skill", "new", "x", "--ns", "ghost"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(10));
}

#[test]
fn skill_new_errors_when_adapter_ambiguous() {
    let h = Harness::new();
    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        "name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[adapters.cursor]\nfiles = [\".cursorrules\"]\n",
    )
    .unwrap();
    let out = h
        .cmd()
        .args(["skill", "new", "x", "--ns", "base"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("adapter"), "stderr = {stderr}");
}
