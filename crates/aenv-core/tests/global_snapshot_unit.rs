//! Unit tests for `aenv_core::global_snapshot::snapshot_global`.
//!
//! The snapshot is the dual of `aenv global activate`: it captures every
//! adapter-managed user-scope path currently under `$HOME` into a new
//! namespace dir that can be re-activated later.

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::error::AenvError;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::global_snapshot::snapshot_global;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use std::path::{Path, PathBuf};

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

fn fake_home() -> &'static Path {
    Path::new("/home/u")
}

fn claude_adapter_with_user_files(user_files: &[&str]) -> Adapter {
    Adapter {
        name: "claude-code".to_string(),
        files: Vec::new(),
        merge_strategies: Default::default(),
        roles: Default::default(),
        default_merge: Default::default(),
        parameters: Vec::new(),
        skills_dir: None,
        soft_limits: Default::default(),
        user_files: user_files.iter().map(|s| s.to_string()).collect(),
        user_roles: Default::default(),
        user_default_merge: Default::default(),
        user_merge_strategies: Default::default(),
        user_soft_limits: Default::default(),
        user_skills_dir: None,
        materialize: None,
    }
}

fn registry_with(adapter: Adapter) -> AdapterRegistry {
    let mut reg = AdapterRegistry::new();
    reg.insert(adapter);
    reg
}

fn read_manifest(fs: &MockFilesystem, layout: &RegistryLayout, name: &str) -> AenvManifest {
    let bytes = fs.read(&layout.manifest_path(name)).unwrap();
    AenvManifest::from_toml(&String::from_utf8(bytes).unwrap()).unwrap()
}

#[test]
fn snapshot_captures_existing_user_files_into_namespace() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let home = fake_home();

    // Seed three user files; the adapter declares the same three.
    fs.write(&home.join(".claude/CLAUDE.md"), b"hello").unwrap();
    fs.write(&home.join(".claude/settings.json"), b"{}")
        .unwrap();
    fs.write(&home.join(".claude/hooks/foo.sh"), b"#!/bin/sh\n")
        .unwrap();

    let adapter = claude_adapter_with_user_files(&[
        "~/.claude/CLAUDE.md",
        "~/.claude/settings.json",
        "~/.claude/hooks/",
    ]);
    let adapters = registry_with(adapter);

    let summary = snapshot_global(&fs, &layout, &adapters, home, "default", &[], false).unwrap();

    // The bytes landed under envs/default/user/.
    let ns_user = layout.namespace_dir("default").join("user");
    assert_eq!(
        fs.read(&ns_user.join(".claude/CLAUDE.md")).unwrap(),
        b"hello"
    );
    assert_eq!(
        fs.read(&ns_user.join(".claude/settings.json")).unwrap(),
        b"{}"
    );
    assert_eq!(
        fs.read(&ns_user.join(".claude/hooks/foo.sh")).unwrap(),
        b"#!/bin/sh\n"
    );

    // The manifest declares every captured path under [adapters.claude-code].
    let manifest = read_manifest(&fs, &layout, "default");
    assert_eq!(manifest.name, "default");
    let entry = manifest
        .adapters
        .get("claude-code")
        .expect("claude-code adapter entry");
    let declared: std::collections::BTreeSet<&str> =
        entry.user_files.iter().map(String::as_str).collect();
    assert!(declared.contains(".claude/CLAUDE.md"));
    assert!(declared.contains(".claude/settings.json"));
    assert!(declared.contains(".claude/hooks/"));

    // Summary counts: two regular files, one directory (hooks/).
    assert_eq!(summary.files_copied, 2);
    assert_eq!(summary.directories_copied, 1);
}

#[test]
fn snapshot_skips_missing_adapter_paths() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let home = fake_home();

    // Only one of the three declared files actually exists on disk.
    fs.write(&home.join(".claude/CLAUDE.md"), b"hello").unwrap();

    let adapter = claude_adapter_with_user_files(&[
        "~/.claude/CLAUDE.md",
        "~/.claude/settings.json",
        "~/.claude/hooks/",
    ]);
    let adapters = registry_with(adapter);

    let summary = snapshot_global(&fs, &layout, &adapters, home, "default", &[], false).unwrap();

    let manifest = read_manifest(&fs, &layout, "default");
    let entry = manifest.adapters.get("claude-code").unwrap();
    assert_eq!(entry.user_files, vec![".claude/CLAUDE.md".to_string()]);
    assert_eq!(summary.files_copied, 1);
    assert_eq!(summary.directories_copied, 0);
}

#[test]
fn snapshot_includes_extra_paths() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let home = fake_home();

    // `.claude/runtime/cli.py` is NOT in the adapter's user_files; the
    // `--include` extra carries it into the snapshot.
    fs.write(&home.join(".claude/runtime/cli.py"), b"print('hi')")
        .unwrap();

    let adapter = claude_adapter_with_user_files(&[]);
    let adapters = registry_with(adapter);

    let summary = snapshot_global(
        &fs,
        &layout,
        &adapters,
        home,
        "snap",
        &[".claude/runtime".to_string()],
        false,
    )
    .unwrap();

    let ns_user = layout.namespace_dir("snap").join("user");
    assert_eq!(
        fs.read(&ns_user.join(".claude/runtime/cli.py")).unwrap(),
        b"print('hi')"
    );
    let manifest = read_manifest(&fs, &layout, "snap");
    let entry = manifest.adapters.get("claude-code").unwrap();
    assert!(entry.user_files.iter().any(|p| p == ".claude/runtime"));
    assert_eq!(summary.directories_copied, 1);
}

#[test]
fn snapshot_refuses_existing_namespace() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let home = fake_home();
    fs.write(&layout.manifest_path("existing"), b"name = \"existing\"\n")
        .unwrap();
    let adapters = registry_with(claude_adapter_with_user_files(&[]));
    let err = snapshot_global(&fs, &layout, &adapters, home, "existing", &[], false)
        .expect_err("must reject existing namespace");
    assert!(
        matches!(err, AenvError::ActivationConflict(_)),
        "expected ActivationConflict, got {err:?}"
    );
}

#[test]
fn snapshot_refuses_invalid_name() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let home = fake_home();
    let adapters = registry_with(claude_adapter_with_user_files(&[]));
    // Colons are rejected by `NamespaceId::new`.
    let err = snapshot_global(&fs, &layout, &adapters, home, "bad:name", &[], false)
        .expect_err("must reject invalid name");
    assert!(
        matches!(err, AenvError::ManifestInvalid(_)),
        "expected ManifestInvalid, got {err:?}"
    );
}

#[test]
fn snapshot_copies_directory_recursively() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let home = fake_home();
    fs.write(&home.join(".claude/hooks/a.sh"), b"a").unwrap();
    fs.write(&home.join(".claude/hooks/b.sh"), b"b").unwrap();
    fs.write(&home.join(".claude/hooks/c.sh"), b"c").unwrap();

    let adapter = claude_adapter_with_user_files(&["~/.claude/hooks/"]);
    let adapters = registry_with(adapter);

    snapshot_global(&fs, &layout, &adapters, home, "dirsnap", &[], false).unwrap();

    let ns_user = layout.namespace_dir("dirsnap").join("user");
    assert_eq!(fs.read(&ns_user.join(".claude/hooks/a.sh")).unwrap(), b"a");
    assert_eq!(fs.read(&ns_user.join(".claude/hooks/b.sh")).unwrap(), b"b");
    assert_eq!(fs.read(&ns_user.join(".claude/hooks/c.sh")).unwrap(), b"c");
}
