//! End-to-end tests for `aenv snapshot <name>`.
//!
//! Each test drives the built `aenv` binary as a subprocess against a real
//! tempdir. Covers: happy path, duplicate-name refusal, --extends, and
//! empty-project refusal.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_aenv")
}

// `ensure_written` always runs with AENV_HOME set to the tempdir, so no manual
// adapter setup is needed.
fn project_with_claude_md(content: &[u8]) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), content).unwrap();
    dir
}

fn assert_success(out: &std::process::Output, ctx: &str) {
    if !out.status.success() {
        panic!(
            "{ctx} failed: status={:?}\nstdout={}\nstderr={}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Read the manifest TOML at `<aenv_home>/envs/<name>/aenv.toml`.
fn read_manifest(aenv_home: &Path, name: &str) -> String {
    std::fs::read_to_string(aenv_home.join(format!("envs/{name}/aenv.toml"))).unwrap()
}

// ---------------------------------------------------------------------------
// Test 1: happy path — project with CLAUDE.md → snapshot → namespace created
// ---------------------------------------------------------------------------

#[test]
fn snapshot_captures_claude_md() {
    let aenv_home = TempDir::new().unwrap();
    let project = project_with_claude_md(b"# Hi from snapshot\n");

    let out = Command::new(bin())
        .args([
            "snapshot",
            "from-proj",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert_success(&out, "snapshot");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("from-proj"),
        "stdout should name the new namespace; got: {stdout}"
    );
    assert!(
        stdout.contains("Snapshotted"),
        "stdout should say Snapshotted; got: {stdout}"
    );

    // Manifest should declare CLAUDE.md under adapters.claude-code.
    let manifest = read_manifest(aenv_home.path(), "from-proj");
    assert!(
        manifest.contains("CLAUDE.md"),
        "manifest should list CLAUDE.md; got:\n{manifest}"
    );
    assert!(
        manifest.contains("claude-code"),
        "manifest should have claude-code adapter; got:\n{manifest}"
    );

    // The actual file should be copied into the namespace dir.
    let ns_claude = aenv_home.path().join("envs/from-proj/CLAUDE.md");
    assert!(
        ns_claude.exists(),
        "CLAUDE.md should exist in namespace dir"
    );
    let content = std::fs::read_to_string(&ns_claude).unwrap();
    assert_eq!(content, "# Hi from snapshot\n");

    // The project pin must NOT be updated (snapshot doesn't pin).
    assert!(
        !project.path().join(".aenv").exists(),
        "snapshot must not write .aenv pin"
    );
}

// ---------------------------------------------------------------------------
// Test 2: duplicate name → exit 12
// ---------------------------------------------------------------------------

#[test]
fn snapshot_refuses_duplicate_name() {
    let aenv_home = TempDir::new().unwrap();
    let project = project_with_claude_md(b"# first\n");

    let out = Command::new(bin())
        .args([
            "snapshot",
            "my-snap",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert_success(&out, "first snapshot");

    // Second invocation with the same name must fail with exit 12.
    let out2 = Command::new(bin())
        .args([
            "snapshot",
            "my-snap",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(
        !out2.status.success(),
        "second snapshot with same name should fail"
    );
    assert_eq!(
        out2.status.code(),
        Some(12),
        "expected exit 12 (ManifestInvalid); got: {:?}\nstderr: {}",
        out2.status.code(),
        String::from_utf8_lossy(&out2.stderr)
    );
    let stderr = String::from_utf8_lossy(&out2.stderr);
    assert!(
        stderr.contains("already exists"),
        "error should mention 'already exists'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: --extends seeds the parent chain in the manifest
// ---------------------------------------------------------------------------

#[test]
fn snapshot_with_extends() {
    let aenv_home = TempDir::new().unwrap();
    let project = project_with_claude_md(b"# extends test\n");

    // Create the "base" namespace first so the registry is aware of it
    // (snapshot itself doesn't validate extends entries, but the manifest
    // content is what we're checking).
    Command::new(bin())
        .args(["create", "base"])
        .env("AENV_HOME", aenv_home.path())
        .status()
        .unwrap();

    let out = Command::new(bin())
        .args([
            "snapshot",
            "child",
            "--project",
            project.path().to_str().unwrap(),
            "--extends",
            "base",
        ])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert_success(&out, "snapshot --extends");

    let manifest = read_manifest(aenv_home.path(), "child");
    assert!(
        manifest.contains("extends"),
        "manifest should have extends field; got:\n{manifest}"
    );
    assert!(
        manifest.contains("base"),
        "manifest extends should list 'base'; got:\n{manifest}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: project with no adapter-managed files → helpful error, no namespace
// ---------------------------------------------------------------------------

#[test]
fn snapshot_refuses_empty_project() {
    let aenv_home = TempDir::new().unwrap();
    // Project has files, but none that any adapter declares.
    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("README.txt"), b"not managed\n").unwrap();

    let out = Command::new(bin())
        .args([
            "snapshot",
            "empty-snap",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "snapshot of empty project should fail"
    );
    // The error falls under ManifestInvalid (exit 12).
    assert_eq!(
        out.status.code(),
        Some(12),
        "expected exit 12; got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no adapter-managed files"),
        "error should mention 'no adapter-managed files'; got: {stderr}"
    );

    // No namespace directory should have been left behind.
    assert!(
        !aenv_home.path().join("envs/empty-snap").exists(),
        "empty namespace dir should be cleaned up"
    );
}
