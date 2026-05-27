//! End-to-end tests for `aenv global snapshot`.
//!
//! Drives the built `aenv` binary as a subprocess with `AENV_HOME` and
//! `HOME` pointed at a `tempfile::tempdir`. Verifies the snapshot is a
//! materializable namespace by round-tripping it through `aenv global
//! activate` and checking for the `Identical` strategy.

use std::path::{Path, PathBuf};
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn global_snapshot_creates_activable_namespace() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"my CLAUDE.md").unwrap();
    std::fs::write(fake_home.join(".claude/settings.json"), b"{}").unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "snapshot", "default"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "snapshot failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // The namespace dir exists and contains the snapshotted files.
    let ns_dir = aenv_home.join("envs/default");
    assert!(ns_dir.join("aenv.toml").exists());
    assert!(ns_dir.join("user/.claude/CLAUDE.md").exists());
    assert_eq!(
        std::fs::read(ns_dir.join("user/.claude/CLAUDE.md")).unwrap(),
        b"my CLAUDE.md"
    );

    // The snapshotted namespace can be activated.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "activate", "default"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "activate of snapshot failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Identical"),
        "expected Identical strategy in activate output, got: {stdout}"
    );
}

#[test]
fn global_snapshot_with_include_captures_extra_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(fake_home.join(".claude/runtime")).unwrap();
    std::fs::write(fake_home.join(".claude/runtime/cli.py"), b"print('hi')").unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args([
            "global",
            "snapshot",
            "with-runtime",
            "--include",
            ".claude/runtime",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "snapshot failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let ns_dir = aenv_home.join("envs/with-runtime");
    assert!(ns_dir.join("user/.claude/runtime/cli.py").exists());
    assert_eq!(
        std::fs::read(ns_dir.join("user/.claude/runtime/cli.py")).unwrap(),
        b"print('hi')"
    );
}
