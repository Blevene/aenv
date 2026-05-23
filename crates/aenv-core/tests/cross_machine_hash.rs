//! Cross-machine hash agreement. Loads fixtures, recomputes each
//! namespace's hash, asserts it matches the line in `expected.txt`.
//!
//! When you need to regenerate `expected.txt`, run:
//!     cargo test -p aenv-core --test cross_machine_hash -- --ignored
//! That fires the ignored `print_hashes` test which dumps the current
//! hashes; copy them verbatim into `expected.txt`.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::hash::hash_resolved_namespace;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::compute_material_set;
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cross_machine")
}

fn copy_fixtures_into_layout(layout: &RegistryLayout) {
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();

    let root = fixtures_root();
    for entry in std::fs::read_dir(&root).unwrap().flatten() {
        if !entry.file_type().unwrap().is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let dest = layout.namespace_dir(&name);
        copy_dir_recursive(&entry.path(), &dest);
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap().flatten() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            // Read + write rather than fs::copy so we can drop CR bytes
            // defensively even if .gitattributes failed somehow.
            let bytes = std::fs::read(&src_path).unwrap();
            let normalized: Vec<u8> = bytes.into_iter().filter(|&b| b != b'\r').collect();
            std::fs::write(&dst_path, normalized).unwrap();
        }
    }
}

fn compute_one(name: &str) -> String {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = RegistryLayout::new(tmp.path().to_path_buf());
    copy_fixtures_into_layout(&layout);
    let fs = aenv_core::fs::RealFilesystem;
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let leaf = NamespaceId::new(name).unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();
    hash_resolved_namespace(&mat)
}

fn expected() -> std::collections::BTreeMap<String, String> {
    let raw = std::fs::read_to_string(fixtures_root().join("expected.txt")).unwrap();
    raw.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|l| {
            let (k, v) = l.split_once('=').expect("expected.txt line: NAME=HASH");
            (k.trim().to_string(), v.trim().to_string())
        })
        .collect()
}

#[test]
fn alpha_hash_matches_fixture() {
    let h = compute_one("alpha");
    let expected = expected();
    assert_eq!(
        h,
        expected.get("alpha").expect("alpha line missing").as_str(),
        "alpha hash drift — regenerate expected.txt or investigate"
    );
}

#[test]
fn beta_hash_matches_fixture() {
    let h = compute_one("beta");
    let expected = expected();
    assert_eq!(
        h,
        expected.get("beta").expect("beta line missing").as_str(),
        "beta hash drift — regenerate expected.txt or investigate"
    );
}

#[test]
#[ignore]
fn print_hashes() {
    println!("alpha={}", compute_one("alpha"));
    println!("beta={}", compute_one("beta"));
}
