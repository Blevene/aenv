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
fn list_prints_all_skills_across_namespaces() {
    let h = Harness::new();
    h.cmd().args(["create", "experiments"]).output().unwrap();
    h.cmd()
        .args(["create", "detailed-execution"])
        .output()
        .unwrap();
    std::fs::write(
        h.aenv_home().join("envs/experiments/aenv.toml"),
        r#"
name = "experiments"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "compare-approaches"
mode = "authored"
adapter = "claude-code"
"#,
    )
    .unwrap();
    std::fs::write(
        h.aenv_home().join("envs/detailed-execution/aenv.toml"),
        r#"
name = "detailed-execution"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "write-tests"
mode = "authored"
adapter = "claude-code"

[[skills]]
name = "match-conventions"
mode = "imported"
adapter = "claude-code"
source = "git+https://example.com/skills.git#match-conventions"
ref = "v1.2.0"
"#,
    )
    .unwrap();

    let out = h.cmd().args(["skill", "list"]).output().unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("experiments"));
    assert!(stdout.contains("compare-approaches"));
    assert!(stdout.contains("write-tests"));
    assert!(stdout.contains("match-conventions"));
    assert!(stdout.contains("v1.2.0"));
    assert!(stdout.contains("authored"));
    assert!(stdout.contains("imported"));
}

#[test]
fn list_filters_by_ns() {
    let h = Harness::new();
    h.cmd().args(["create", "a"]).output().unwrap();
    h.cmd().args(["create", "b"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/a/aenv.toml"),
        "name = \"a\"\n\n[[skills]]\nname = \"alpha\"\nmode = \"authored\"\n",
    )
    .unwrap();
    std::fs::write(
        h.aenv_home().join("envs/b/aenv.toml"),
        "name = \"b\"\n\n[[skills]]\nname = \"beta\"\nmode = \"authored\"\n",
    )
    .unwrap();

    let out = h
        .cmd()
        .args(["skill", "list", "--ns", "a"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("alpha"));
    assert!(!stdout.contains("beta"));
}
