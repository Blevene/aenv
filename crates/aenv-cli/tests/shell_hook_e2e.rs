//! E2E tests for `aenv init-shell` + `aenv activate-if-needed`.
//!
//! Covers the four state transitions the shell hook drives:
//!   none → none, none → project, project → same project, project → other, project → none.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

/// Run `aenv` from a specific cwd with the given args + AENV_HOME.
fn run_in(aenv_home: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .env("AENV_HOME", aenv_home)
        .output()
        .expect("aenv ran")
}

fn aenv_home_with_starters() -> TempDir {
    let home = TempDir::new().unwrap();
    // First invocation populates ~/.aenv with built-in adapters + starters.
    let out = Command::new(bin())
        .args(["list"])
        .env("AENV_HOME", home.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    home
}

#[test]
fn init_shell_emits_bash_zsh_fish_scripts() {
    let home = aenv_home_with_starters();
    for shell in &["bash", "zsh", "fish"] {
        let out = Command::new(bin())
            .args(["init-shell", shell])
            .env("AENV_HOME", home.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "init-shell {shell} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let script = String::from_utf8(out.stdout).unwrap();
        assert!(
            script.contains("activate-if-needed"),
            "{shell} script must call activate-if-needed"
        );
        assert!(
            script.contains("_AENV_ACTIVE"),
            "{shell} script must use _AENV_ACTIVE state var"
        );
    }
}

#[test]
fn init_shell_rejects_unknown_shell() {
    let home = aenv_home_with_starters();
    let out = Command::new(bin())
        .args(["init-shell", "tcsh"])
        .env("AENV_HOME", home.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("supported"));
}

#[test]
fn activate_if_needed_none_to_project_activates() {
    let home = aenv_home_with_starters();
    let proj = TempDir::new().unwrap();
    // Pin the project to karpathy first.
    let pin = run_in(home.path(), proj.path(), &["use", "karpathy"]);
    assert!(pin.status.success());

    // Empty last-active + cwd inside the pinned project → activates.
    let out = run_in(home.path(), proj.path(), &["activate-if-needed", ""]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = String::from_utf8(out.stdout).unwrap();
    // stdout is the canonical project path; canonicalize ours for comparison.
    let canonical = std::fs::canonicalize(proj.path()).unwrap();
    assert_eq!(
        printed.trim(),
        canonical.display().to_string(),
        "should print the activated project root"
    );
    assert!(
        proj.path().join(".aenv-state/state.json").exists(),
        "activate-if-needed should have written state.json"
    );
}

#[test]
fn activate_if_needed_same_project_is_noop() {
    let home = aenv_home_with_starters();
    let proj = TempDir::new().unwrap();
    let canonical = std::fs::canonicalize(proj.path()).unwrap();

    run_in(home.path(), proj.path(), &["use", "karpathy"]);
    run_in(home.path(), proj.path(), &["activate-if-needed", ""]);
    let state_path = proj.path().join(".aenv-state/state.json");
    let mtime_before = std::fs::metadata(&state_path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));

    // Second invocation with last-active = same project → fast path, no work.
    let out = run_in(
        home.path(),
        proj.path(),
        &["activate-if-needed", &canonical.display().to_string()],
    );
    assert!(out.status.success());
    let mtime_after = std::fs::metadata(&state_path).unwrap().modified().unwrap();
    assert_eq!(
        mtime_before, mtime_after,
        "fast path must not rewrite state.json"
    );
}

#[test]
fn activate_if_needed_project_to_none_deactivates() {
    let home = aenv_home_with_starters();
    let proj = TempDir::new().unwrap();
    let canonical = std::fs::canonicalize(proj.path()).unwrap();
    let not_pinned = TempDir::new().unwrap();

    run_in(home.path(), proj.path(), &["use", "karpathy"]);
    run_in(home.path(), proj.path(), &["activate-if-needed", ""]);
    assert!(proj.path().join(".aenv-state/state.json").exists());

    // Now invoke from a directory that has no .aenv anywhere upstream.
    // The hook would set last-active to the proj path we just left.
    let out = run_in(
        home.path(),
        not_pinned.path(),
        &["activate-if-needed", &canonical.display().to_string()],
    );
    assert!(out.status.success());
    let printed = String::from_utf8(out.stdout).unwrap();
    assert_eq!(printed.trim(), "", "no scope → empty stdout");
    assert!(
        !proj.path().join(".aenv-state/state.json").exists(),
        "leaving scope should have deactivated"
    );
}

#[test]
fn activate_if_needed_project_a_to_project_b_transitions() {
    let home = aenv_home_with_starters();
    let proj_a = TempDir::new().unwrap();
    let proj_b = TempDir::new().unwrap();
    let canonical_a = std::fs::canonicalize(proj_a.path()).unwrap();
    let canonical_b = std::fs::canonicalize(proj_b.path()).unwrap();

    run_in(home.path(), proj_a.path(), &["use", "karpathy"]);
    run_in(home.path(), proj_b.path(), &["use", "cherny"]);
    run_in(home.path(), proj_a.path(), &["activate-if-needed", ""]);
    assert!(proj_a.path().join(".aenv-state/state.json").exists());

    // cd from A to B with last-active = A: should deactivate A + activate B.
    let out = run_in(
        home.path(),
        proj_b.path(),
        &["activate-if-needed", &canonical_a.display().to_string()],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = String::from_utf8(out.stdout).unwrap();
    assert_eq!(printed.trim(), canonical_b.display().to_string());
    assert!(
        !proj_a.path().join(".aenv-state/state.json").exists(),
        "A should be deactivated"
    );
    assert!(
        proj_b.path().join(".aenv-state/state.json").exists(),
        "B should be activated"
    );
}
