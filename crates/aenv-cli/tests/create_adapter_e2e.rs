//! E2E tests for `aenv create --adapter`.

use std::process::Command;
use tempfile::TempDir;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

#[test]
fn create_with_adapter_scaffolds_usable_namespace() {
    // `aenv create --adapter claude-code` should produce a namespace that
    // works out of the box: the manifest declares the file(s) the adapter
    // manages, AND those files exist (empty) on disk so `aenv activate`
    // materializes something the user can edit.
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

    let ns_dir = aenv_home.path().join("envs/foo");
    let text = std::fs::read_to_string(ns_dir.join("aenv.toml")).unwrap();
    assert!(
        text.contains("[adapters.claude-code]"),
        "expected [adapters.claude-code] in manifest: {text}"
    );
    // Claude-code declares `["CLAUDE.md", ".claude/"]`. The trailing-slash
    // entry is a directory marker (user populates via `aenv skill new`), so
    // only CLAUDE.md gets scaffolded.
    assert!(
        text.contains("files = [\"CLAUDE.md\"]"),
        "should declare files = [\"CLAUDE.md\"]: {text}"
    );
    let claude_md = ns_dir.join("CLAUDE.md");
    assert!(claude_md.exists(), "CLAUDE.md should be scaffolded on disk");
    assert_eq!(
        std::fs::metadata(&claude_md).unwrap().len(),
        0,
        "scaffolded CLAUDE.md should be empty — user fills it in"
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

    let ns_dir = aenv_home.path().join("envs/foo");
    let text = std::fs::read_to_string(ns_dir.join("aenv.toml")).unwrap();
    assert!(
        text.contains("[adapters.claude-code]"),
        "expected [adapters.claude-code]: {text}"
    );
    assert!(
        text.contains("[adapters.cursor]"),
        "expected [adapters.cursor]: {text}"
    );
    // Both adapters' concrete files should be scaffolded.
    assert!(ns_dir.join("CLAUDE.md").exists(), "claude-code CLAUDE.md");
    assert!(ns_dir.join(".cursorrules").exists(), "cursor .cursorrules");
}
