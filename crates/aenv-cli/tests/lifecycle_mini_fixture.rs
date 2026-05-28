//! Mini lifecycle fixture — proves `on_activate` can pip-install a real
//! Python package, and that a script that bails out before invoking pip
//! still rolls back the materialization cleanly.
//!
//! Requires `python3` + `pip` available on PATH. Marked `#[ignore]` so CI
//! without Python doesn't fail; run locally with `--ignored`:
//!
//! ```bash
//! cargo test -p aenv-cli --test lifecycle_mini_fixture -- --ignored
//! ```

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

fn aenv(aenv_home: &Path, fake_home: &Path) -> Command {
    let mut c = Command::new(bin());
    c.env("AENV_HOME", aenv_home).env("HOME", fake_home);
    c
}

fn canon(p: impl AsRef<Path>) -> PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

/// Set executable bits on a unix script.
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
        && Command::new("python3")
            .args(["-m", "pip", "--version"])
            .output()
            .is_ok_and(|o| o.status.success())
}

/// Seed a minimal claude-code adapter declaring one user file.
fn seed_adapter(aenv_home: &Path) {
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
}

/// Seed a `mini` namespace with a `runtime/` Python package and an
/// `install.sh` lifecycle script that `pip install --user -e`s it. The
/// `body` argument is the full shell-script body to write (a hook for
/// the failure-variant test to swap in a bailing script).
fn seed_mini_namespace(aenv_home: &Path, script_body: &str) {
    let ns_dir = aenv_home.join("envs/mini");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(
        ns_dir.join("user/.claude/CLAUDE.md"),
        b"mini namespace user-scope file\n",
    )
    .unwrap();

    // Minimal Python package under runtime/. We use setup.py (rather
    // than pyproject.toml + [project]) because older setuptools versions
    // ship with `[project]` support disabled, which prevents editable
    // installs from picking up the package name. setup.py works across
    // pip/setuptools versions back to ~Python 3.6.
    let pkg = ns_dir.join("runtime/aenv_test_pkg");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(pkg.join("__init__.py"), b"").unwrap();
    std::fs::write(
        ns_dir.join("runtime/setup.py"),
        r#"from setuptools import setup
setup(name="aenv-test-pkg", version="0.0.0", packages=["aenv_test_pkg"])
"#,
    )
    .unwrap();

    let script = ns_dir.join("install.sh");
    std::fs::write(&script, script_body).unwrap();
    make_executable(&script);

    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "mini"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "install.sh"
"#,
    )
    .unwrap();
}

#[test]
#[ignore = "requires python3 + pip; run with --ignored locally"]
fn on_activate_pip_install_succeeds_and_module_importable() {
    if !python3_available() {
        eprintln!("python3 + pip not on PATH; skipping");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();

    seed_adapter(&aenv_home);
    seed_mini_namespace(
        &aenv_home,
        r#"#!/usr/bin/env bash
set -euo pipefail
cd "$AENV_NAMESPACE_DIR/runtime"
python3 -m pip install --user -e . > /dev/null 2>&1
echo "aenv_test_pkg installed"
"#,
    );

    // Use --yes so the SHA-pinned approval prompt doesn't block.
    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "mini", "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global activate failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Activation materialized the user file.
    let materialized = fake_home.join(".claude/CLAUDE.md");
    assert!(materialized.exists(), "missing {materialized:?}");

    // Activation also wrote the global state file.
    assert!(
        aenv_home.join("global-state.json").exists(),
        "global-state.json not written"
    );

    // The on_activate stdout should have surfaced.
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("aenv_test_pkg installed"),
        "did not see install marker in stdout: {stdout}"
    );

    // Module is importable. Use the same python3 the script used; HOME
    // override means `pip install --user` puts the package under
    // $fake_home/.local/... so we pass HOME to python3 too.
    let py = Command::new("python3")
        .env("HOME", &fake_home)
        .args(["-c", "import aenv_test_pkg; print('ok')"])
        .output()
        .unwrap();
    assert!(
        py.status.success(),
        "python3 -c 'import aenv_test_pkg' failed: stdout={} stderr={}",
        String::from_utf8_lossy(&py.stdout),
        String::from_utf8_lossy(&py.stderr)
    );

    // Deactivate: no `on_deactivate` in this minimal fixture, so the
    // pip-installed module stays — that's the user-responsibility
    // cleanup boundary documented in pm_docs/lifecycle-hooks.md §6.
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

    // The materialized user file is gone.
    assert!(
        !materialized.exists(),
        "expected materialized CLAUDE.md to be removed after deactivate"
    );
}

#[test]
#[ignore = "requires python3 + pip; run with --ignored locally"]
fn on_activate_failure_rolls_back_pip_state() {
    if !python3_available() {
        eprintln!("python3 + pip not on PATH; skipping");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();

    seed_adapter(&aenv_home);
    // Script bails BEFORE invoking pip — proves rollback works without
    // leaking pip state because pip is never actually called.
    seed_mini_namespace(
        &aenv_home,
        r#"#!/usr/bin/env bash
set -euo pipefail
echo "about to fail before pip"
exit 1
"#,
    );

    // Pre-existing user file that activation MUST stash and restore.
    let user_file = fake_home.join(".claude/CLAUDE.md");
    std::fs::create_dir_all(user_file.parent().unwrap()).unwrap();
    std::fs::write(&user_file, b"PRE-EXISTING USER CONTENT\n").unwrap();

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "mini", "--yes"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "expected activate to fail; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // GlobalConflict = exit 19.
    assert_eq!(
        out.status.code(),
        Some(19),
        "expected exit code 19 (GlobalConflict); stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The pre-existing user file is restored byte-for-byte.
    let restored = std::fs::read(&user_file).expect("user file should exist after rollback");
    assert_eq!(
        restored, b"PRE-EXISTING USER CONTENT\n",
        "pre-existing user file was not restored on rollback"
    );

    // No global-state.json got written.
    assert!(
        !aenv_home.join("global-state.json").exists(),
        "global-state.json should not exist after a rolled-back activation"
    );
}
