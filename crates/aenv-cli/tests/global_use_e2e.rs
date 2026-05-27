//! End-to-end tests for `aenv global use` and `aenv global deactivate`.
//!
//! Drives the built `aenv` binary as a subprocess with `AENV_HOME` and
//! `HOME` pointed at a `tempfile::tempdir`. Exercises the user-scope
//! activation surface in isolation from the real `$HOME`.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

fn aenv(aenv_home: &Path, fake_home: &Path) -> Command {
    let mut c = Command::new(bin());
    c.env("AENV_HOME", aenv_home).env("HOME", fake_home);
    c
}

/// Create the minimal `claude-code` adapter (with a single `~/.claude/CLAUDE.md`
/// user file) plus a namespace `ns` whose `user/.claude/CLAUDE.md` payload is
/// the literal bytes `new`.
fn seed_minimal_user_scope(aenv_home: &Path) {
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();
}

#[test]
fn global_use_activates_user_files_under_home_override() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_minimal_user_scope(&aenv_home);

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "use", "ns"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global use failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let materialized = fake_home.join(".claude/CLAUDE.md");
    assert!(materialized.exists(), "missing {materialized:?}");
    assert_eq!(std::fs::read(&materialized).unwrap(), b"new");
    assert!(aenv_home.join("global-state.json").exists());

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ns"),
        "stdout did not mention namespace name: {stdout}"
    );
    assert!(
        stdout.contains("running harness sessions"),
        "stdout missing running-session caveat: {stdout}"
    );
}
