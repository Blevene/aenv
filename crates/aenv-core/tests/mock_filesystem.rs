//! Tests for `MockFilesystem` — verifies it honors the same `Filesystem`
//! contract as `RealFilesystem` for the operations Phase 1 will rely on.

use aenv_core::fs::{FileKind, Filesystem, MockFilesystem};
use std::path::PathBuf;

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[test]
fn empty_mock_has_nothing() {
    let fs = MockFilesystem::new();
    assert!(!fs.exists(&p("/anything")).unwrap());
}

#[test]
fn write_then_read_roundtrip() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/a/b/c.txt"), b"hello").unwrap();
    assert_eq!(fs.read(&p("/a/b/c.txt")).unwrap(), b"hello");
}

#[test]
fn write_auto_creates_parent_dirs() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/a/b/c.txt"), b"x").unwrap();
    let meta = fs.metadata(&p("/a/b")).unwrap();
    assert_eq!(meta.kind, FileKind::Directory);
}

#[test]
fn rename_moves_file() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/from"), b"data").unwrap();
    fs.rename(&p("/from"), &p("/to")).unwrap();
    assert!(!fs.exists(&p("/from")).unwrap());
    assert_eq!(fs.read(&p("/to")).unwrap(), b"data");
}

#[test]
fn remove_file_deletes() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/x"), b"x").unwrap();
    fs.remove_file(&p("/x")).unwrap();
    assert!(!fs.exists(&p("/x")).unwrap());
}

#[test]
fn remove_dir_all_deletes_tree() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/a/b/c"), b"x").unwrap();
    fs.write(&p("/a/d"), b"y").unwrap();
    fs.remove_dir_all(&p("/a")).unwrap();
    assert!(!fs.exists(&p("/a")).unwrap());
    assert!(!fs.exists(&p("/a/b/c")).unwrap());
}

#[test]
fn symlink_records_target() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/target"), b"t").unwrap();
    fs.symlink(&p("/target"), &p("/link")).unwrap();
    assert!(fs.is_symlink(&p("/link")).unwrap());
    assert_eq!(fs.read_link(&p("/link")).unwrap(), p("/target"));
}

#[test]
fn read_follows_symlink() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/target"), b"t").unwrap();
    fs.symlink(&p("/target"), &p("/link")).unwrap();
    assert_eq!(fs.read(&p("/link")).unwrap(), b"t");
}

#[test]
fn symlink_metadata_reports_symlink_kind_not_target() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/target"), b"t").unwrap();
    fs.symlink(&p("/target"), &p("/link")).unwrap();
    assert_eq!(fs.metadata(&p("/link")).unwrap().kind, FileKind::File);
    assert_eq!(
        fs.symlink_metadata(&p("/link")).unwrap().kind,
        FileKind::Symlink
    );
}

#[test]
fn list_dir_returns_immediate_children() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/d/a"), b"x").unwrap();
    fs.write(&p("/d/b"), b"y").unwrap();
    fs.create_dir_all(&p("/d/sub")).unwrap();

    let mut entries: Vec<PathBuf> = fs.list_dir(&p("/d")).unwrap();
    entries.sort();
    assert_eq!(entries, vec![p("/d/a"), p("/d/b"), p("/d/sub")]);
}

#[test]
fn injected_failures_propagate() {
    // The mock supports per-path failure injection so Phase 1 can test
    // mid-activation IO errors.
    let mut fs = MockFilesystem::new();
    fs.fail_writes_to(&p("/cursed"));
    let result = fs.write(&p("/cursed"), b"x");
    assert!(result.is_err(), "expected injected failure");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Other);
}
