//! End-to-end tests for `aenv doctor [<namespace>]`.
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
// Test 1: clean namespace with no violations
// ---------------------------------------------------------------------------

#[test]
fn doctor_reports_clean_when_no_violations() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = 5000
"#,
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/base/CLAUDE.md"), "short body").unwrap();

    let out = h.cmd().args(["doctor", "base"]).output().unwrap();

    let s = stdout(&out);
    assert!(
        out.status.success(),
        "expected exit 0; status={:?}, stdout={s}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        s.contains("Namespace 'base'"),
        "expected 'Namespace 'base''; got: {s:?}"
    );
    assert!(
        s.contains("No issues found"),
        "expected 'No issues found'; got: {s:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: advisory violation — exit 0 but report shows POLICY
// ---------------------------------------------------------------------------

#[test]
fn doctor_reports_advisory_violation_zero_exit() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    let body = "x".repeat(8000);
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = 5000
"#,
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/base/CLAUDE.md"), body).unwrap();

    let out = h.cmd().args(["doctor", "base"]).output().unwrap();

    let s = stdout(&out);
    assert!(
        out.status.success(),
        "expected exit 0 (advisory only); status={:?}, stdout={s}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        s.contains("POLICY"),
        "expected 'POLICY' in output; got: {s:?}"
    );
    assert!(
        s.contains("instructions_max_chars"),
        "expected 'instructions_max_chars'; got: {s:?}"
    );
    assert!(s.contains("8000"), "expected '8000' in output; got: {s:?}");
}

// ---------------------------------------------------------------------------
// Test 3: enforce violation — exit 17
// ---------------------------------------------------------------------------

#[test]
fn doctor_exits_17_on_enforce_violation() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "tight"]).output().unwrap();
    assert_success(&out, "create tight");

    let body = "x".repeat(8000);
    std::fs::write(
        h.aenv_home().join("envs/tight/aenv.toml"),
        r#"
name = "tight"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = { value = 5000, enforce = true }
"#,
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/tight/CLAUDE.md"), body).unwrap();

    let out = h.cmd().args(["doctor", "tight"]).output().unwrap();

    let s = stdout(&out);
    assert_eq!(
        out.status.code(),
        Some(17),
        "expected exit code 17; status={:?}, stdout={s}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        s.contains("instructions_max_chars"),
        "expected 'instructions_max_chars' in output; got: {s:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: no arg — uses active project's pinned namespace
// ---------------------------------------------------------------------------

#[test]
fn doctor_with_no_arg_uses_active_project() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/base/CLAUDE.md"), "ok").unwrap();

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
        .args(["doctor"])
        .current_dir(h.project())
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
        s.contains("Namespace 'base'"),
        "expected 'Namespace 'base''; got: {s:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 5: no arg, no pin — exit 20
// ---------------------------------------------------------------------------

#[test]
fn doctor_with_no_arg_no_pin_exits_20() {
    let h = Harness::new();

    let out = h
        .cmd()
        .args(["doctor"])
        .current_dir(h.project())
        .output()
        .unwrap();

    assert_eq!(
        out.status.code(),
        Some(20),
        "expected exit code 20 (project not pinned); status={:?}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}
