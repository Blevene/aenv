//! Integration tests for `aenv --version` and `aenv -V`.

use std::process::Command;

fn bin() -> std::path::PathBuf {
    // CARGO_BIN_EXE_<name> is set by cargo for integration tests.
    env!("CARGO_BIN_EXE_aenv").into()
}

#[test]
fn version_long_flag_prints_crate_version() {
    let output = Command::new(bin())
        .arg("--version")
        .output()
        .expect("failed to run aenv --version");
    assert!(output.status.success(), "expected success, got {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout not utf-8");
    let expected = format!("aenv {}", env!("CARGO_PKG_VERSION"));
    assert!(
        stdout.trim() == expected,
        "expected {:?}, got {:?}",
        expected,
        stdout.trim()
    );
}

#[test]
fn version_short_flag_prints_crate_version() {
    let output = Command::new(bin())
        .arg("-V")
        .output()
        .expect("failed to run aenv -V");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout not utf-8");
    let expected = format!("aenv {}", env!("CARGO_PKG_VERSION"));
    assert_eq!(stdout.trim(), expected);
}
