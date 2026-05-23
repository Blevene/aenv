//! E2E tests for `aenv create --adapter`.

use std::process::Command;
use tempfile::TempDir;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

#[test]
fn create_with_adapter_seeds_block() {
    let aenv_home = TempDir::new().unwrap();
    let out = Command::new(bin())
        .args(["create", "foo", "--adapter", "claude-code"])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let manifest = aenv_home.path().join("envs/foo/aenv.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(
        text.contains("[adapters.claude-code]"),
        "expected [adapters.claude-code] in manifest: {text}"
    );
    assert!(
        text.contains("files = []"),
        "should seed empty files vec: {text}"
    );
}

#[test]
fn create_with_unknown_adapter_errors() {
    let aenv_home = TempDir::new().unwrap();
    let out = Command::new(bin())
        .args(["create", "foo", "--adapter", "not-a-real-adapter"])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(!out.status.success(), "should fail for unknown adapter");
    assert_eq!(
        out.status.code(),
        Some(11),
        "AdapterMissing should exit 11, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // No namespace directory should have been written.
    assert!(
        !aenv_home.path().join("envs/foo").exists(),
        "namespace dir should not be created on error"
    );
}

#[test]
fn create_with_multiple_adapters_seeds_all_blocks() {
    let aenv_home = TempDir::new().unwrap();
    // Install a second adapter so the registry knows about it.
    // We use the built-in claude-code; for a second one we use a temp toml file.
    let adapter_dir = aenv_home.path().join("adapters");
    std::fs::create_dir_all(&adapter_dir).unwrap();
    std::fs::write(
        adapter_dir.join("cursor.toml"),
        b"name = \"cursor\"\nfiles = [\".cursorrules\"]\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "create",
            "foo",
            "--adapter",
            "claude-code",
            "--adapter",
            "cursor",
        ])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let manifest = aenv_home.path().join("envs/foo/aenv.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(
        text.contains("[adapters.claude-code]"),
        "expected [adapters.claude-code]: {text}"
    );
    assert!(
        text.contains("[adapters.cursor]"),
        "expected [adapters.cursor]: {text}"
    );
}
