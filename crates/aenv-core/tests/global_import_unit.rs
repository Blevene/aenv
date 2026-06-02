//! Unit tests for `aenv_core::global_snapshot::import_global`.
//!
//! The importer turns a source directory tree into a namespace. It picks
//! between an authoritative `aenv-namespace.toml` convention file at the
//! source root and a fixed heuristic that recognizes `claude-ctrl`-style
//! layouts.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::AenvError;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::global_snapshot::import_global;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use std::path::{Path, PathBuf};

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

fn fake_source() -> &'static Path {
    Path::new("/src")
}

fn empty_registry() -> AdapterRegistry {
    AdapterRegistry::new()
}

fn seed_dir(fs: &MockFilesystem, path: &Path) {
    fs.create_dir_all(path).unwrap();
}

fn read_manifest_str(fs: &MockFilesystem, layout: &RegistryLayout, name: &str) -> String {
    let bytes = fs.read(&layout.manifest_path(name)).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn read_manifest(fs: &MockFilesystem, layout: &RegistryLayout, name: &str) -> AenvManifest {
    AenvManifest::from_toml(&read_manifest_str(fs, layout, name)).unwrap()
}

#[test]
fn import_with_convention_file_uses_explicit_layout() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let src = fake_source();
    seed_dir(&fs, src);

    let conv = r#"adapters = ["claude-code"]

[layout]
"myrules/" = ".claude/myrules/"
"top.md" = ".claude/top.md"
"#;
    fs.write(&src.join("aenv-namespace.toml"), conv.as_bytes())
        .unwrap();
    fs.write(&src.join("myrules/a.md"), b"rule a").unwrap();
    fs.write(&src.join("myrules/b.md"), b"rule b").unwrap();
    fs.write(&src.join("top.md"), b"top").unwrap();

    let summary = import_global(&fs, &layout, &empty_registry(), src, "ns1", false).unwrap();
    assert!(
        summary.convention_file_used,
        "convention_file_used must be true"
    );

    let user_root = layout.namespace_dir("ns1").join("user");
    assert_eq!(
        fs.read(&user_root.join(".claude/myrules/a.md")).unwrap(),
        b"rule a"
    );
    assert_eq!(fs.read(&user_root.join(".claude/top.md")).unwrap(), b"top");

    let manifest = read_manifest(&fs, &layout, "ns1");
    let entry = manifest.adapters.get("claude-code").unwrap();
    let declared: std::collections::BTreeSet<&str> =
        entry.user_files.iter().map(String::as_str).collect();
    assert!(declared.contains(".claude/myrules/"));
    assert!(declared.contains(".claude/top.md"));
}

#[test]
fn import_heuristic_recognizes_claude_ctrl_layout() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let src = fake_source();
    seed_dir(&fs, src);
    fs.write(&src.join("CLAUDE.md"), b"# hello").unwrap();
    fs.write(&src.join("agents/x.md"), b"agent x").unwrap();
    fs.write(&src.join("hooks/y.sh"), b"#!/bin/sh\n").unwrap();
    fs.write(&src.join("install.sh"), b"#!/bin/sh\necho install\n")
        .unwrap();

    let summary =
        import_global(&fs, &layout, &empty_registry(), src, "claude-cntrl", false).unwrap();
    assert!(!summary.convention_file_used);

    let user_root = layout.namespace_dir("claude-cntrl").join("user");
    assert_eq!(
        fs.read(&user_root.join(".claude/CLAUDE.md")).unwrap(),
        b"# hello"
    );
    assert_eq!(
        fs.read(&user_root.join(".claude/agents/x.md")).unwrap(),
        b"agent x"
    );
    assert_eq!(
        fs.read(&user_root.join(".claude/hooks/y.sh")).unwrap(),
        b"#!/bin/sh\n"
    );

    let manifest = read_manifest(&fs, &layout, "claude-cntrl");
    let entry = manifest.adapters.get("claude-code").unwrap();
    let declared: std::collections::BTreeSet<&str> =
        entry.user_files.iter().map(String::as_str).collect();
    assert!(declared.contains(".claude/CLAUDE.md"));
    assert!(declared.contains(".claude/agents/"));
    assert!(declared.contains(".claude/hooks/"));

    // The heuristic does NOT infer lifecycle hooks from a repo's install.sh:
    // such scripts are self-installers that fight aenv's materialization.
    // No [lifecycle] section, and install.sh is not copied into the namespace
    // (it's only copied when a script is actually wired).
    let ns_dir = layout.namespace_dir("claude-cntrl");
    assert!(
        !fs.exists(&ns_dir.join("install.sh")).unwrap(),
        "heuristic should not copy install.sh into the namespace"
    );
    let raw = read_manifest_str(&fs, &layout, "claude-cntrl");
    assert!(
        !raw.contains("[lifecycle]"),
        "heuristic must not infer a [lifecycle] block, got:\n{raw}"
    );
}

#[test]
fn import_skips_paths_in_ignore() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let src = fake_source();
    seed_dir(&fs, src);
    let conv = r#"
ignore = ["docs/"]

[layout]
"docs/" = ".claude/docs/"
"CLAUDE.md" = ".claude/CLAUDE.md"
"#;
    fs.write(&src.join("aenv-namespace.toml"), conv.as_bytes())
        .unwrap();
    fs.write(&src.join("docs/dontcopy.md"), b"docs").unwrap();
    fs.write(&src.join("CLAUDE.md"), b"hello").unwrap();

    import_global(&fs, &layout, &empty_registry(), src, "ns2", false).unwrap();

    let user_root = layout.namespace_dir("ns2").join("user");
    assert_eq!(
        fs.read(&user_root.join(".claude/CLAUDE.md")).unwrap(),
        b"hello"
    );
    assert!(
        !fs.exists(&user_root.join(".claude/docs")).unwrap(),
        ".claude/docs/ must not be present"
    );
    assert!(
        !fs.exists(&user_root.join(".claude/docs/dontcopy.md"))
            .unwrap(),
        "ignored file must not be present"
    );
}

#[test]
fn import_refuses_existing_namespace() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let src = fake_source();
    seed_dir(&fs, src);
    fs.write(&src.join("CLAUDE.md"), b"x").unwrap();
    fs.write(&layout.manifest_path("existing"), b"name = \"existing\"\n")
        .unwrap();

    let err = import_global(&fs, &layout, &empty_registry(), src, "existing", false)
        .expect_err("must reject existing namespace");
    assert!(
        matches!(err, AenvError::ActivationConflict(_)),
        "expected ActivationConflict, got {err:?}"
    );
}

#[test]
fn import_refuses_invalid_name() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let src = fake_source();
    seed_dir(&fs, src);
    let err = import_global(&fs, &layout, &empty_registry(), src, "bad:name", false)
        .expect_err("must reject invalid name");
    assert!(
        matches!(err, AenvError::ManifestInvalid(_)),
        "expected ManifestInvalid, got {err:?}"
    );
}

#[test]
fn import_refuses_non_existent_source() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let err = import_global(
        &fs,
        &layout,
        &empty_registry(),
        Path::new("/does/not/exist"),
        "ns",
        false,
    )
    .expect_err("must reject missing source");
    assert!(
        matches!(err, AenvError::ManifestInvalid(_)),
        "expected ManifestInvalid, got {err:?}"
    );
}

#[test]
fn import_refuses_file_source() {
    let fs = MockFilesystem::new();
    let layout = layout();
    fs.write(Path::new("/not-a-dir.md"), b"x").unwrap();
    let err = import_global(
        &fs,
        &layout,
        &empty_registry(),
        Path::new("/not-a-dir.md"),
        "ns",
        false,
    )
    .expect_err("must reject a regular file as source");
    assert!(
        matches!(err, AenvError::ManifestInvalid(_)),
        "expected ManifestInvalid, got {err:?}"
    );
}

#[test]
fn import_buckets_codex_paths_under_codex_adapter() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let src = fake_source();
    seed_dir(&fs, src);
    fs.write(&src.join(".codex/AGENTS.md"), b"agents").unwrap();
    fs.write(&src.join("CLAUDE.md"), b"claude").unwrap();

    import_global(&fs, &layout, &empty_registry(), src, "mixed", false).unwrap();

    let manifest = read_manifest(&fs, &layout, "mixed");
    let claude_entry = manifest
        .adapters
        .get("claude-code")
        .expect("claude-code entry");
    assert!(claude_entry
        .user_files
        .iter()
        .any(|p| p == ".claude/CLAUDE.md"));
    let codex_entry = manifest.adapters.get("codex").expect("codex entry");
    // The heuristic maps `.codex/` -> `.codex/` (directory), so the entry is
    // the directory itself (trailing slash preserved), not individual files.
    assert!(codex_entry.user_files.iter().any(|p| p == ".codex/"));
    // And the file actually ended up on disk under the namespace.
    let user_root = layout.namespace_dir("mixed").join("user");
    assert_eq!(
        fs.read(&user_root.join(".codex/AGENTS.md")).unwrap(),
        b"agents"
    );
    // No cross-contamination.
    assert!(!claude_entry
        .user_files
        .iter()
        .any(|p| p.starts_with(".codex/")));
    assert!(!codex_entry
        .user_files
        .iter()
        .any(|p| p.starts_with(".claude/")));
}

#[test]
fn import_with_convention_file_writes_lifecycle_section() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let src = fake_source();
    seed_dir(&fs, src);
    let conv = r#"
[lifecycle]
on_activate = "install.sh"
on_deactivate = "uninstall.sh"

[layout]
"CLAUDE.md" = ".claude/CLAUDE.md"
"#;
    fs.write(&src.join("aenv-namespace.toml"), conv.as_bytes())
        .unwrap();
    fs.write(&src.join("CLAUDE.md"), b"hi").unwrap();
    fs.write(&src.join("install.sh"), b"#!/bin/sh\n").unwrap();
    fs.write(&src.join("uninstall.sh"), b"#!/bin/sh\n").unwrap();

    let summary = import_global(&fs, &layout, &empty_registry(), src, "with-lc", false).unwrap();
    assert!(summary.convention_file_used);

    let ns_dir = layout.namespace_dir("with-lc");
    assert_eq!(fs.read(&ns_dir.join("install.sh")).unwrap(), b"#!/bin/sh\n");
    assert_eq!(
        fs.read(&ns_dir.join("uninstall.sh")).unwrap(),
        b"#!/bin/sh\n"
    );

    let raw = read_manifest_str(&fs, &layout, "with-lc");
    assert!(raw.contains("[lifecycle]"));
    assert!(raw.contains("on_activate = \"install.sh\""));
    assert!(raw.contains("on_deactivate = \"uninstall.sh\""));

    // AenvManifest parses the file successfully (unknown section tolerated).
    let _ = read_manifest(&fs, &layout, "with-lc");
}
