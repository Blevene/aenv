//! Regression: `aenv global snapshot` walks user_files trees that contain
//! symlinks pointing to directories.
//!
//! The original `copy_dir_all` saw `FileKind::Symlink`, took the file branch,
//! called `fs.read()` on the symlink — which follows it — and got EISDIR
//! when the target was a directory (e.g. `~/.claude/plugins/cache/firebase/
//! firebase/1.1.0/skills` was a symlink to a sibling dir). Whole snapshot
//! aborted with `io error: Is a directory (os error 21)`.
//!
//! Fix: when the candidate is a symlink, resolve to the target's kind and
//! dispatch — Directory → recurse, File → read+write, broken → skip+warn.

#![cfg(unix)]

use std::path::Path;

#[test]
fn snapshot_walks_through_symlink_to_directory_inside_user_files_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    // Adapter declaring `~/.claude/plugins/` as a managed user_files entry.
    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/plugins/"]
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    // Build a plugins tree with:
    //   plugins/pkg/version/data/         — real dir with content
    //   plugins/pkg/version/skills        — SYMLINK to plugins/pkg/version/data
    //   plugins/pkg/version/broken        — broken SYMLINK
    let pkg = fake_home.join(".claude/plugins/pkg/version");
    std::fs::create_dir_all(pkg.join("data")).unwrap();
    std::fs::write(pkg.join("data/inner.json"), b"inner").unwrap();
    std::fs::write(pkg.join("manifest.json"), b"manifest").unwrap();
    // The reported claude-ctrl plugin-cache pattern: a relative symlink
    // pointing at a sibling directory.
    std::os::unix::fs::symlink(Path::new("data"), pkg.join("skills")).unwrap();
    std::os::unix::fs::symlink(Path::new("does-not-exist"), pkg.join("broken")).unwrap();

    let summary = aenv_core::global_snapshot::snapshot_global(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        "snap",
        &[],
        false,
    )
    .unwrap();

    // The top-level `plugins/` counts as one captured directory. Files
    // inside captured directories are not folded into `files_copied` per
    // the snapshot summary contract (counts top-level units only) — we
    // verify their presence on disk below instead.
    assert_eq!(summary.directories_copied, 1);
    assert_eq!(summary.files_copied, 0);

    // The symlink target's content shows up under the captured tree.
    let captured_root = aenv_home.join("envs/snap/user/.claude/plugins/pkg/version");
    assert!(captured_root.join("manifest.json").exists());
    assert_eq!(
        std::fs::read(captured_root.join("manifest.json")).unwrap(),
        b"manifest"
    );
    assert!(captured_root.join("data/inner.json").exists());
    // skills/ (the symlink) was followed and copied as a regular directory
    // with the same inner.json.
    assert!(captured_root.join("skills/inner.json").exists());
    assert_eq!(
        std::fs::read(captured_root.join("skills/inner.json")).unwrap(),
        b"inner"
    );
    // Broken symlink was skipped (no broken/ in the captured tree).
    assert!(!captured_root.join("broken").exists());
}

#[test]
fn snapshot_handles_top_level_user_files_entry_that_is_a_symlink_to_a_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/agents/"]
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    // The adapter says ~/.claude/agents/. The user has a symlink there
    // pointing at a sibling real dir.
    let real_dir = fake_home.join(".claude/real-agents");
    std::fs::create_dir_all(&real_dir).unwrap();
    std::fs::write(real_dir.join("foo.md"), b"agent body").unwrap();
    std::os::unix::fs::symlink(Path::new("real-agents"), fake_home.join(".claude/agents")).unwrap();

    let summary = aenv_core::global_snapshot::snapshot_global(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        "snap",
        &[],
        false,
    )
    .unwrap();
    assert_eq!(summary.directories_copied, 1);
    let captured = aenv_home.join("envs/snap/user/.claude/agents/foo.md");
    assert!(captured.exists(), "expected captured at {captured:?}");
    assert_eq!(std::fs::read(captured).unwrap(), b"agent body");
}
