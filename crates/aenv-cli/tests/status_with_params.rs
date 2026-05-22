//! End-to-end tests for `aenv status` with parameters and policies sections.
//! Uses the raw std::process::Command + Harness pattern (no assert_cmd).

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: PathBuf,
    project: PathBuf,
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

fn assert_success(out: &std::process::Output, ctx: &str) {
    if !out.status.success() {
        panic!(
            "{ctx} failed: status={:?}, stdout={}, stderr={}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// ---------------------------------------------------------------------------
// Test: status shows parameters and active policies sections
// ---------------------------------------------------------------------------

#[test]
fn status_prints_parameters_section() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[parameters]
default_model = "haiku"
budget = 5000

[policies]
skill_requires_description = true
"#,
    )
    .unwrap();

    // Pin and activate the project.
    let out = h
        .cmd()
        .args(["use", "base", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use base");

    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "activate");

    let out = h
        .cmd()
        .args(["status", "--project"])
        .arg(h.project())
        .output()
        .unwrap();

    let s = stdout(&out);
    assert!(
        out.status.success(),
        "expected exit 0; status={:?}, stdout={s}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        s.contains("Parameters"),
        "expected 'Parameters' section; got: {s:?}"
    );
    assert!(
        s.contains("default_model"),
        "expected 'default_model' in output; got: {s:?}"
    );
    assert!(
        s.contains("haiku"),
        "expected 'haiku' in output; got: {s:?}"
    );
    assert!(
        s.contains("budget"),
        "expected 'budget' in output; got: {s:?}"
    );
    assert!(
        s.contains("Active policies"),
        "expected 'Active policies' section; got: {s:?}"
    );
    assert!(
        s.contains("skill_requires_description"),
        "expected 'skill_requires_description' in output; got: {s:?}"
    );
}
