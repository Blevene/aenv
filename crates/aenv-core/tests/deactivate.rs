//! Tests for deactivate_namespace.

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::deactivate::deactivate_namespace;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::create_namespace;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

fn registry_with_claude() -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    r.insert(Adapter {
        name: "claude-code".to_string(),
        files: vec!["CLAUDE.md".to_string()],
        merge_strategies: Default::default(),
    });
    r
}

fn setup_namespace(fs: &MockFilesystem, ns: &str, body: &[u8]) {
    let layout = layout();
    create_namespace(fs, &layout, ns).unwrap();
    fs.write(
        &layout.manifest_path(ns),
        format!("name = \"{ns}\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n").as_bytes(),
    )
    .unwrap();
    fs.write(&layout.namespace_dir(ns).join("CLAUDE.md"), body)
        .unwrap();
}

#[test]
fn deactivate_removes_symlink_and_state() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"disposition");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        "experiments",
    )
    .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
    assert!(!fs.exists(&project.join(".aenv/state.json")).unwrap());
}

#[test]
fn deactivate_restores_backed_up_originals() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"namespace");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), b"original").unwrap();
    activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        "experiments",
    )
    .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    let restored = fs.read(&project.join("CLAUDE.md")).unwrap();
    assert_eq!(restored, b"original");
    assert!(!fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn deactivate_leaves_unmanaged_files_alone() {
    // R-48: aenv removes only files it materialized.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"x");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        "experiments",
    )
    .unwrap();

    // User creates a file during activation.
    fs.write(&project.join("README.md"), b"user file").unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    // Symlink removed; user file untouched.
    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
    assert_eq!(fs.read(&project.join("README.md")).unwrap(), b"user file");
}

#[test]
fn deactivate_errors_when_no_state_file() {
    // "No active state to deactivate" is distinct from "no .aenv pin":
    // a user can have a pin (project is associated with a namespace) but
    // have never activated. ActivationConflict (exit 13) is the right
    // variant; ProjectNotPinned (exit 20) is specifically about the
    // pin file itself missing.
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = deactivate_namespace(&fs, &project).expect_err("must error");
    assert!(matches!(err, aenv_core::AenvError::ActivationConflict(_)));
    assert_eq!(err.exit_code(), 13);
}

#[test]
fn deactivate_leaves_identical_file_in_place() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let body: &[u8] = b"shared";
    setup_namespace(&fs, "experiments", body);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), body).unwrap(); // identical
    activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        "experiments",
    )
    .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    // Identical-strategy file is the user's; it stays.
    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), body);
}
