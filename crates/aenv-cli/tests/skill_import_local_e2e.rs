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
fn import_local_path_adds_manifest_entry() {
    let h = Harness::new();
    let src = tempdir().unwrap();
    std::fs::write(
        src.path().join("SKILL.md"),
        "---\nname: external\ndescription: x\n---\n",
    )
    .unwrap();

    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        "name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let canonical_source = std::fs::canonicalize(src.path()).unwrap();
    let out = h
        .cmd()
        .args(["skill", "import"])
        .arg(canonical_source.to_str().unwrap())
        .args(["--ns", "base", "--adapter", "claude-code"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(manifest.contains("[[skills]]"));
    assert!(manifest.contains("mode = \"imported\""));
    assert!(manifest.contains(canonical_source.to_str().unwrap()));
}
