//! End-to-end tests for `aenv set <ns>.<param> <value>`.
//! Uses the raw std::process::Command + Harness pattern (no assert_cmd).

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    aenv_home: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        Self {
            _aenv_home_guard: aenv_home_guard,
            aenv_home,
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

// ---------------------------------------------------------------------------
// Test 1: set inserts a new parameter
// ---------------------------------------------------------------------------

#[test]
fn set_inserts_new_parameter() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    let out = h
        .cmd()
        .args(["set", "base.default_model", "claude-opus-4.7"])
        .output()
        .unwrap();
    assert_success(&out, "set base.default_model");

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(
        manifest.contains("default_model"),
        "manifest missing 'default_model': {manifest}"
    );
    assert!(
        manifest.contains("claude-opus-4.7"),
        "manifest missing value: {manifest}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: set overwrites an existing parameter
// ---------------------------------------------------------------------------

#[test]
fn set_overwrites_existing() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    // Write a manifest that already declares budget = 5000.
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        b"name = \"base\"\n\n[parameters]\nbudget = 5000\n",
    )
    .unwrap();

    let out = h
        .cmd()
        .args(["set", "base.budget", "3000"])
        .output()
        .unwrap();
    assert_success(&out, "set base.budget");

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(
        manifest.contains("3000"),
        "manifest should contain 3000: {manifest}"
    );
    assert!(
        !manifest.contains("5000"),
        "manifest should no longer contain 5000: {manifest}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: set infers boolean
// ---------------------------------------------------------------------------

#[test]
fn set_infers_boolean() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    let out = h
        .cmd()
        .args(["set", "base.verbose", "true"])
        .output()
        .unwrap();
    assert_success(&out, "set base.verbose");

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(
        manifest.contains("verbose = true"),
        "manifest should contain 'verbose = true': {manifest}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: set infers list of strings
// ---------------------------------------------------------------------------

#[test]
fn set_infers_list_of_strings() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    let out = h
        .cmd()
        .args(["set", "base.forbid_tools", "[edit, write, bash:rm]"])
        .output()
        .unwrap();
    assert_success(&out, "set base.forbid_tools");

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(manifest.contains("edit"), "missing 'edit': {manifest}");
    assert!(manifest.contains("write"), "missing 'write': {manifest}");
    assert!(
        manifest.contains("bash:rm"),
        "missing 'bash:rm': {manifest}"
    );
}

// ---------------------------------------------------------------------------
// Test 5: unknown namespace exits 10
// ---------------------------------------------------------------------------

#[test]
fn set_unknown_namespace_exits_10() {
    let h = Harness::new();

    let out = h.cmd().args(["set", "ghost.x", "1"]).output().unwrap();
    assert!(
        !out.status.success(),
        "expected failure for unknown namespace"
    );
    assert_eq!(
        out.status.code(),
        Some(10),
        "expected exit code 10 (NamespaceNotFound); stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------
// Test 6: set rejects leading-dot form (active-project spec)
// ---------------------------------------------------------------------------

#[test]
fn set_requires_explicit_namespace() {
    let h = Harness::new();

    let out = h.cmd().args(["set", ".x", "1"]).output().unwrap();
    assert!(
        !out.status.success(),
        "expected failure when no explicit namespace given"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("namespace"),
        "stderr should mention 'namespace'; got: {stderr:?}"
    );
}
