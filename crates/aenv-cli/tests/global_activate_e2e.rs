//! End-to-end tests for `aenv global activate` and `aenv global deactivate`.
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
fn global_activate_materializes_user_files_under_home_override() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_minimal_user_scope(&aenv_home);

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global activate failed: status={:?}, stdout={}, stderr={}",
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

#[test]
fn global_deactivate_restores_stash() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original").unwrap();
    seed_minimal_user_scope(&aenv_home);

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global activate failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global deactivate failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"original",
        "original CLAUDE.md not restored after deactivate"
    );
    assert!(
        !aenv_home.join("global-state.json").exists(),
        "global-state.json should be removed after deactivate"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ns"),
        "deactivate stdout should mention namespace name: {stdout}"
    );
}

#[test]
fn use_with_global_flag_activates_both_scopes() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    let project = std::fs::canonicalize(tmp.path()).unwrap().join("project");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
files = ["CLAUDE.md"]
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/both");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("CLAUDE.md"), b"project body").unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"user body").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "both"
[adapters.claude-code]
files = ["CLAUDE.md"]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let out = aenv(&aenv_home, &fake_home)
        .args([
            "use",
            "both",
            "--global",
            "--project",
            project.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // `aenv use --global` is sugar for: pin the project, activate it,
    // and activate globally. All three side effects must land.
    assert!(project.join(".aenv").exists(), "project not pinned");

    // Project-scope materialization: CLAUDE.md should exist under the
    // project root (this is the activate step that previously was missing).
    let project_claude = project.join("CLAUDE.md");
    assert!(
        project_claude.exists(),
        "project CLAUDE.md not materialized by --global sugar: {project_claude:?}"
    );
    assert_eq!(std::fs::read(&project_claude).unwrap(), b"project body");

    // User-scope materialization: $HOME/.claude/CLAUDE.md.
    let user_claude = fake_home.join(".claude/CLAUDE.md");
    assert!(
        user_claude.exists(),
        "user CLAUDE.md not materialized: {user_claude:?}"
    );
    assert_eq!(std::fs::read(&user_claude).unwrap(), b"user body");
    assert!(aenv_home.join("global-state.json").exists());
}

#[test]
fn global_deactivate_with_nothing_active_is_ok() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global deactivate with no activation should succeed: status={:?}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no global activation"),
        "expected no-op message, got: {stdout}"
    );
}
