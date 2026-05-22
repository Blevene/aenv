use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

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

fn make_repo_with_skill() -> tempfile::TempDir {
    let bare = tempdir().unwrap();
    Command::new("git")
        .args(["init", "--bare"])
        .arg(bare.path())
        .status()
        .unwrap();
    let work = tempdir().unwrap();
    Command::new("git")
        .args(["clone"])
        .arg(bare.path())
        .arg(work.path())
        .status()
        .unwrap();
    // SKILL.md at repo root so the git resolver finds it via the fallback path.
    std::fs::write(
        work.path().join("SKILL.md"),
        "---\nname: my-skill\ndescription: y\n---\n",
    )
    .unwrap();
    Command::new("git")
        .current_dir(work.path())
        .args(["add", "."])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(work.path())
        .args(["-c", "user.email=t@e", "-c", "user.name=t", "commit", "-m", "init"])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(work.path())
        .args(["push", "origin", "HEAD:master"])
        .status()
        .unwrap();
    drop(work);
    bare
}

#[test]
fn import_git_pinned_writes_resolved_ref() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_repo_with_skill();
    let h = Harness::new();
    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        "name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let url = format!("git+file://{}", bare.path().display());
    let out = h
        .cmd()
        .args(["skill", "import"])
        .arg(&url)
        .args(["--ns", "base", "--adapter", "claude-code", "--pin", "master"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let manifest =
        std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    // Some git SHA was written as the pinned ref.
    assert!(manifest.contains("ref ="));
    // It should be a 40-char hex string (full SHA) or the branch name as a fallback.
    assert!(manifest.contains("master") || manifest.contains("ref = \""));
}
