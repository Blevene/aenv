//! Integration tests for `RealFilesystem` against a real `tempfile::tempdir`.
//!
//! These tests pin the `Filesystem` trait surface to operations that Phase 1
//! materialization actually performs.

use aenv_core::fs::{FileKind, Filesystem, RealFilesystem};
use std::path::PathBuf;
use tempfile::tempdir;

fn rfs() -> RealFilesystem {
    RealFilesystem
}

#[test]
fn write_then_read_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("hello.txt");
    let mut fs = rfs();

    fs.write(&path, b"hello world").unwrap();
    let read = fs.read(&path).unwrap();
    assert_eq!(read, b"hello world");
}

#[test]
fn exists_returns_ok_false_for_missing() {
    let dir = tempdir().unwrap();
    let fs = rfs();
    assert!(!fs.exists(&dir.path().join("nope")).unwrap());
}

#[test]
fn exists_returns_ok_true_after_write() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("file");
    let mut fs = rfs();
    fs.write(&path, b"x").unwrap();
    assert!(fs.exists(&path).unwrap());
}

#[test]
fn create_dir_all_is_idempotent() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("a/b/c");
    let mut fs = rfs();
    fs.create_dir_all(&nested).unwrap();
    fs.create_dir_all(&nested).unwrap(); // second call is a no-op
    assert!(fs.exists(&nested).unwrap());
}

#[cfg(unix)]
#[test]
fn symlink_then_read_link_roundtrip() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");
    let mut fs = rfs();

    fs.write(&target, b"target contents").unwrap();
    fs.symlink(&target, &link).unwrap();

    assert!(fs.is_symlink(&link).unwrap());
    assert_eq!(fs.read_link(&link).unwrap(), target);

    // Reading through the symlink returns the target contents.
    assert_eq!(fs.read(&link).unwrap(), b"target contents");
}

#[test]
fn rename_moves_file() {
    let dir = tempdir().unwrap();
    let from = dir.path().join("a");
    let to = dir.path().join("b");
    let mut fs = rfs();

    fs.write(&from, b"data").unwrap();
    fs.rename(&from, &to).unwrap();

    assert!(!fs.exists(&from).unwrap());
    assert!(fs.exists(&to).unwrap());
}

#[test]
fn remove_file_deletes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("kill-me");
    let mut fs = rfs();
    fs.write(&path, b"x").unwrap();
    fs.remove_file(&path).unwrap();
    assert!(!fs.exists(&path).unwrap());
}

#[test]
fn remove_dir_all_deletes_tree() {
    let dir = tempdir().unwrap();
    let tree = dir.path().join("a/b/c");
    let mut fs = rfs();
    fs.create_dir_all(&tree).unwrap();
    fs.write(&tree.join("leaf"), b"x").unwrap();

    fs.remove_dir_all(&dir.path().join("a")).unwrap();
    assert!(!fs.exists(&tree).unwrap());
}

#[test]
fn write_auto_creates_parent_directories() {
    // Part of the Filesystem trait contract: write() creates missing
    // parents. Phase 1 materialization depends on this.
    let dir = tempdir().unwrap();
    let deep = dir.path().join("a/b/c/leaf.txt");
    let mut fs = rfs();
    fs.write(&deep, b"hi").unwrap();
    assert_eq!(fs.read(&deep).unwrap(), b"hi");
}

#[cfg(unix)]
#[test]
fn symlink_metadata_reports_symlink_kind_not_target() {
    // metadata() follows symlinks; symlink_metadata() does not. Phase 1
    // activation logic relies on this distinction to detect aenv-managed
    // symlinks without backing up the underlying target file.
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");
    let mut fs = rfs();

    fs.write(&target, b"target contents").unwrap();
    fs.symlink(&target, &link).unwrap();

    assert_eq!(fs.metadata(&link).unwrap().kind, FileKind::File);
    assert_eq!(fs.symlink_metadata(&link).unwrap().kind, FileKind::Symlink);
}

#[test]
fn metadata_reports_kind_and_size() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("f");
    let mut fs = rfs();
    fs.write(&file, b"abcd").unwrap();

    let meta = fs.metadata(&file).unwrap();
    assert_eq!(meta.kind, FileKind::File);
    assert_eq!(meta.len, 4);
}

#[test]
fn metadata_distinguishes_directory_from_file() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("d");
    let mut fs = rfs();
    fs.create_dir_all(&nested).unwrap();

    let meta = fs.metadata(&nested).unwrap();
    assert_eq!(meta.kind, FileKind::Directory);
}

#[test]
fn list_dir_returns_immediate_children() {
    let dir = tempdir().unwrap();
    let mut fs = rfs();
    fs.write(&dir.path().join("a"), b"x").unwrap();
    fs.write(&dir.path().join("b"), b"y").unwrap();
    fs.create_dir_all(&dir.path().join("c")).unwrap();

    let mut entries: Vec<PathBuf> = fs.list_dir(dir.path()).unwrap();
    entries.sort();
    let expected: Vec<PathBuf> = ["a", "b", "c"].iter().map(|n| dir.path().join(n)).collect();
    assert_eq!(entries, expected);
}
