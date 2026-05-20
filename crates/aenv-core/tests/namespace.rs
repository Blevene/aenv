//! Tests for namespace registry operations.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::namespace::{create_namespace, delete_namespace, list_namespaces};
use aenv_core::AenvError;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

#[test]
fn create_writes_default_manifest() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();

    let manifest_bytes = fs.read(&layout.manifest_path("experiments")).unwrap();
    let m = AenvManifest::from_toml(&String::from_utf8(manifest_bytes).unwrap()).unwrap();
    assert_eq!(m.name, "experiments");
    assert!(m.adapters.is_empty());
}

#[test]
fn create_rejects_duplicate() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();
    let err = create_namespace(&fs, &layout, "experiments").expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn list_returns_empty_when_no_namespaces() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert!(names.is_empty());
}

#[test]
fn list_returns_namespace_names_sorted() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();
    create_namespace(&fs, &layout, "analyst").unwrap();
    create_namespace(&fs, &layout, "detailed-execution").unwrap();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert_eq!(
        names,
        vec![
            "analyst".to_string(),
            "detailed-execution".to_string(),
            "experiments".to_string(),
        ]
    );
}

#[test]
fn list_skips_entries_without_manifest() {
    // A stray directory under envs/ that lacks aenv.toml is not a namespace.
    // list_namespaces silently ignores it.
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "real").unwrap();
    fs.create_dir_all(&layout.namespaces_dir().join("stray"))
        .unwrap();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert_eq!(names, vec!["real".to_string()]);
}

#[test]
fn delete_removes_namespace_directory() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();
    delete_namespace(&fs, &layout, "experiments").unwrap();
    assert!(!fs.exists(&layout.namespace_dir("experiments")).unwrap());
}

#[test]
fn delete_rejects_unknown_namespace() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let err = delete_namespace(&fs, &layout, "nope").expect_err("must error");
    assert!(matches!(err, AenvError::NamespaceNotFound(_)));
    assert_eq!(err.exit_code(), 10);
}
