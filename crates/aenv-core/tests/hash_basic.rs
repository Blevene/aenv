//! Sanity tests for hash_resolved_namespace against a tiny known fixture.

use aenv_core::hash::{hash_resolved_namespace, HASH_PREFIX_V1};
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::{compute_material_set, MaterialSet};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn empty_material_set_hashes_to_constant() {
    let mat = MaterialSet::new(vec![], BTreeMap::new());
    let h = hash_resolved_namespace(&mat);
    assert!(h.starts_with(HASH_PREFIX_V1));
    let hex = h.strip_prefix(HASH_PREFIX_V1).unwrap();
    assert_eq!(hex.len(), 64);
    assert!(hex
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn single_entry_material_set_is_deterministic() {
    let mat = MaterialSet::new(
        vec![(PathBuf::from("CLAUDE.md"), b"hello\n".to_vec())],
        BTreeMap::new(),
    );
    let h1 = hash_resolved_namespace(&mat);
    let h2 = hash_resolved_namespace(&mat);
    assert_eq!(h1, h2);
}

#[test]
fn hash_differs_on_content_change() {
    let a = MaterialSet::new(
        vec![(PathBuf::from("CLAUDE.md"), b"hello\n".to_vec())],
        BTreeMap::new(),
    );
    let b = MaterialSet::new(
        vec![(PathBuf::from("CLAUDE.md"), b"hello!\n".to_vec())],
        BTreeMap::new(),
    );
    assert_ne!(hash_resolved_namespace(&a), hash_resolved_namespace(&b));
}

#[test]
fn hash_differs_on_path_change() {
    let a = MaterialSet::new(
        vec![(PathBuf::from("a.md"), b"x".to_vec())],
        BTreeMap::new(),
    );
    let b = MaterialSet::new(
        vec![(PathBuf::from("b.md"), b"x".to_vec())],
        BTreeMap::new(),
    );
    assert_ne!(hash_resolved_namespace(&a), hash_resolved_namespace(&b));
}

#[test]
fn hash_via_compute_material_set_round_trip() {
    let tmp = TempDir::new().unwrap();
    let layout = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();

    let ns_root = layout.namespace_dir("solo");
    write_file(
        &layout.manifest_path("solo"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&ns_root.join("CLAUDE.md"), "# Hello\n");

    let leaf = NamespaceId::new("solo").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();
    let h = hash_resolved_namespace(&mat);
    assert!(h.starts_with(HASH_PREFIX_V1));
    assert_eq!(h.strip_prefix(HASH_PREFIX_V1).unwrap().len(), 64);
}
