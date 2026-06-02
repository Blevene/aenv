//! End-to-end tests for the unified scope flags (issue #5, Layer 1):
//! `aenv create --global`, `aenv activate --global`, `aenv deactivate --global`.
//!
//! These route to the same user-scope core as the `aenv global …` tree, which
//! stays as a backward-compatible alias. Driven as a subprocess with `AENV_HOME`
//! and `HOME` pointed at tempdirs so the real `$HOME` is never touched.

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

/// `aenv create --global <ns>` scaffolds a user-scope namespace (the same as
/// `aenv global new`): a manifest with `user_files` and content under `user/`.
#[test]
fn create_global_scaffolds_user_scope_namespace() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");

    let out = aenv(&aenv_home, fake_home.path())
        .args(["create", "myprof", "--global"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "create --global failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let manifest = std::fs::read_to_string(aenv_home.join("envs/myprof/aenv.toml")).unwrap();
    assert!(
        manifest.contains("user_files"),
        "manifest should declare user_files; got:\n{manifest}"
    );
    assert!(
        aenv_home
            .join("envs/myprof/user/.claude/CLAUDE.md")
            .exists(),
        "user-scope content should be scaffolded under user/"
    );
}

/// The unified verbs drive the same user-scope state the `aenv global …` tree
/// reports: `create --global` → `activate --global` → seen by `global status`
/// → `deactivate --global` clears it.
#[test]
fn unified_global_activate_deactivate_round_trip() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");

    aenv(&aenv_home, fake_home.path())
        .args(["create", "myprof", "--global"])
        .status()
        .unwrap();

    // Activate via the unified verb.
    let act = aenv(&aenv_home, fake_home.path())
        .args(["activate", "myprof", "--global", "--yes"])
        .output()
        .unwrap();
    assert!(
        act.status.success(),
        "activate --global failed: {}",
        String::from_utf8_lossy(&act.stderr)
    );

    // The materialized file is a real symlink in the fake HOME.
    let claude_md = fake_home.path().join(".claude/CLAUDE.md");
    assert!(
        std::fs::symlink_metadata(&claude_md)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "~/.claude/CLAUDE.md should be a symlink after activate --global"
    );

    // The global tree sees the same active state.
    let status = aenv(&aenv_home, fake_home.path())
        .args(["global", "status"])
        .output()
        .unwrap();
    let status_out = String::from_utf8_lossy(&status.stdout);
    assert!(
        status_out.contains("Active global namespace: myprof"),
        "global status should report myprof active; got:\n{status_out}"
    );

    // Deactivate via the unified verb.
    let deact = aenv(&aenv_home, fake_home.path())
        .args(["deactivate", "--global"])
        .output()
        .unwrap();
    assert!(
        deact.status.success(),
        "deactivate --global failed: {}",
        String::from_utf8_lossy(&deact.stderr)
    );
    assert!(
        !claude_md.exists(),
        "symlink should be gone after deactivate --global"
    );
}

/// `aenv global activate` / `global deactivate` still work as the alias — a
/// namespace activated through the global tree is torn down by the unified
/// verb and vice versa (one shared state).
#[test]
fn unified_and_global_tree_share_state() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");

    aenv(&aenv_home, fake_home.path())
        .args(["create", "myprof", "--global"])
        .status()
        .unwrap();

    // Activate through the (deprecated) global tree…
    let act = aenv(&aenv_home, fake_home.path())
        .args(["global", "activate", "myprof", "--yes"])
        .output()
        .unwrap();
    assert!(act.status.success());

    // …and tear it down with the unified verb.
    let deact = aenv(&aenv_home, fake_home.path())
        .args(["deactivate", "--global"])
        .output()
        .unwrap();
    assert!(
        deact.status.success(),
        "unified deactivate should clear a global-tree activation: {}",
        String::from_utf8_lossy(&deact.stderr)
    );
    let status = aenv(&aenv_home, fake_home.path())
        .args(["global", "status"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&status.stdout).contains("no global activation"),
        "expected no active global namespace after unified deactivate"
    );
}

/// Cross-flag guards: scope-specific flags must error when misused, before
/// touching anything.
#[test]
fn scope_flag_guards_error() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");

    let cases: &[(&[&str], &str)] = &[
        (&["activate", "--global"], "needs a namespace name"),
        (&["activate", "--yes"], "only apply with --global"),
        (&["deactivate", "--global", "--prune"], "project scope only"),
        (&["deactivate", "--force"], "only applies with --global"),
        (
            &["create", "p2", "--global", "--extends", "base"],
            "not supported with --global",
        ),
    ];

    for (args, needle) in cases {
        let out = aenv(&aenv_home, fake_home.path())
            .args(*args)
            .output()
            .unwrap();
        assert!(
            !out.status.success(),
            "expected failure for `aenv {}`",
            args.join(" ")
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains(needle),
            "`aenv {}` stderr should contain {needle:?}; got: {stderr}",
            args.join(" ")
        );
    }
}
