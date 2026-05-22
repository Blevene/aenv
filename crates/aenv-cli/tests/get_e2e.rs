//! End-to-end tests for `aenv get <spec>`.
//!
//! Uses the same raw `std::process::Command` + `Harness` pattern as
//! `composition_e2e.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Harness (same shape as composition_e2e.rs)
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
    String::from_utf8(out.stdout.clone()).unwrap()
}

// ---------------------------------------------------------------------------
// Test 1: get active-project parameter via .<param>
// ---------------------------------------------------------------------------

#[test]
fn get_active_project_parameter() {
    let h = Harness::new();

    // Create namespace and write manifest with a parameter.
    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        b"name = \"base\"\n[parameters]\ndefault_model = \"claude-haiku-4.5\"\n",
    )
    .unwrap();

    // Pin and activate the project so state.json exists.
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

    // Run `aenv get .default_model` with project set via cwd.
    let out = h
        .cmd()
        .args(["get", ".default_model"])
        .current_dir(h.project())
        .output()
        .unwrap();
    assert_success(&out, "get .default_model");

    let s = stdout(&out);
    assert!(
        s.contains("claude-haiku-4.5"),
        "expected value 'claude-haiku-4.5'; got: {s:?}"
    );
    assert!(
        s.contains("source: base"),
        "expected 'source: base'; got: {s:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: explicit namespace spec shows override provenance
// ---------------------------------------------------------------------------

#[test]
fn get_with_explicit_namespace_shows_override_provenance() {
    let h = Harness::new();

    // Create base and leaf namespaces.
    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");
    let out = h.cmd().args(["create", "leaf"]).output().unwrap();
    assert_success(&out, "create leaf");

    // base declares default_model = "claude-haiku-4.5"
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        b"name = \"base\"\n[parameters]\ndefault_model = \"claude-haiku-4.5\"\n",
    )
    .unwrap();

    // leaf extends base and overrides default_model = "claude-opus-4.7"
    std::fs::write(
        h.aenv_home().join("envs/leaf/aenv.toml"),
        b"name = \"leaf\"\nextends = [\"base\"]\n[parameters]\ndefault_model = \"claude-opus-4.7\"\n",
    )
    .unwrap();

    // No activation needed for explicit ns.param form.
    let out = h
        .cmd()
        .args(["get", "leaf.default_model"])
        .output()
        .unwrap();
    assert_success(&out, "get leaf.default_model");

    let s = stdout(&out);
    assert!(
        s.contains("claude-opus-4.7"),
        "expected value 'claude-opus-4.7'; got: {s:?}"
    );
    assert!(
        s.contains("source: leaf"),
        "expected 'source: leaf'; got: {s:?}"
    );
    assert!(
        s.contains("overrides base"),
        "expected 'overrides base' in provenance; got: {s:?}"
    );
    assert!(
        s.contains("claude-haiku-4.5"),
        "expected prior value 'claude-haiku-4.5' in provenance; got: {s:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: undefined parameter exits 16
// ---------------------------------------------------------------------------

#[test]
fn get_undefined_parameter_exits_16() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    // Manifest with no parameters.
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        b"name = \"base\"\n",
    )
    .unwrap();

    let out = h.cmd().args(["get", "base.nonexistent"]).output().unwrap();

    assert!(!out.status.success(), "should have failed");
    assert_eq!(
        out.status.code(),
        Some(16),
        "expected exit code 16 for undefined parameter; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Test 4: active-project form with no state file exits 20
// ---------------------------------------------------------------------------

#[test]
fn get_active_when_no_project_pinned_exits_20() {
    let h = Harness::new();

    // No `.aenv` pin, no `.aenv-state/` in the project dir.
    // Run get from the bare project dir (no .aenv file → find_project_root fails → exit 20).
    let out = h
        .cmd()
        .args(["get", ".default_model"])
        .current_dir(h.project())
        .output()
        .unwrap();

    assert!(!out.status.success(), "should have failed");
    assert_eq!(
        out.status.code(),
        Some(20),
        "expected exit code 20 (project not pinned); stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}
