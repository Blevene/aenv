//! End-to-end tests for orphan-stash detection and `aenv global deactivate --prune`.
//!
//! An orphan stash is a timestamped subdir of `<aenv_home>/global-stash/` that
//! is not referenced by the currently-active `global-state.json`. With no
//! active state, every stash subdir is orphan.

use std::path::Path;
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn global_doctor_reports_orphan_stash() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    // Fabricate an orphan stash by hand: stash dir with content, no state file.
    std::fs::create_dir_all(aenv_home.join("global-stash/epoch-99/.claude")).unwrap();
    std::fs::write(
        aenv_home.join("global-stash/epoch-99/.claude/CLAUDE.md"),
        b"abandoned",
    )
    .unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "doctor"])
        .output()
        .unwrap();
    // No namespace argument → orphan stash should make this fail with exit 19.
    assert_eq!(
        out.status.code(),
        Some(19),
        "expected exit 19, got {:?}; stderr={}; stdout={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("orphan"),
        "expected mention of orphan stash: {combined}"
    );
}

#[test]
fn global_deactivate_prune_removes_orphan_stash() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let orphan = aenv_home.join("global-stash/epoch-77/.claude");
    std::fs::create_dir_all(&orphan).unwrap();
    std::fs::write(orphan.join("CLAUDE.md"), b"abandoned").unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "deactivate", "--prune"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The orphan stash dir is gone.
    assert!(
        !aenv_home.join("global-stash/epoch-77").exists(),
        "orphan stash should have been pruned"
    );
}

#[test]
fn list_orphan_stashes_returns_empty_when_no_stash_root() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    std::fs::create_dir_all(&aenv_home).unwrap();
    let layout = aenv_core::home::RegistryLayout::new(aenv_home);
    let orphans = aenv_core::state::list_orphan_stashes(&layout).unwrap();
    assert!(orphans.is_empty());
}
