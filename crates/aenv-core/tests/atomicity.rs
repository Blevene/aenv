//! Tests for the rename atomicity probe.

use aenv_core::atomicity::probe_rename_atomicity;
use aenv_core::fs::{Filesystem, MockFilesystem};
use std::path::PathBuf;

#[test]
fn probe_succeeds_on_clean_aenv_dir() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    probe_rename_atomicity(&fs, &project).unwrap();
}

#[test]
fn probe_creates_aenv_dir_if_absent() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    probe_rename_atomicity(&fs, &project).unwrap();
    assert!(fs.exists(&project.join(".aenv")).unwrap());
}

#[test]
fn probe_leaves_no_probe_files_behind() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    probe_rename_atomicity(&fs, &project).unwrap();
    let entries = fs.list_dir(&project.join(".aenv")).unwrap();
    // Probe should leave .aenv/ empty (or containing nothing it created).
    assert!(entries.is_empty(), "found leftover entries: {entries:?}");
}
