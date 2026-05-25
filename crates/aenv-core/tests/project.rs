//! Tests for `.aenv` pin file IO and project-root resolution.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::project::{find_project_root, read_pin, write_pin};
use aenv_core::AenvError;
use std::path::{Path, PathBuf};

#[test]
fn write_then_read_pin_roundtrip() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/payments-api");
    write_pin(&fs, &project, "detailed-execution").unwrap();
    let pin = read_pin(&fs, &project).unwrap();
    assert_eq!(pin, "detailed-execution");
}

#[test]
fn read_pin_errors_when_missing() {
    let fs = MockFilesystem::new();
    let err = read_pin(&fs, Path::new("/projects/missing")).expect_err("must error");
    assert!(matches!(err, AenvError::ProjectNotPinned));
    assert_eq!(err.exit_code(), 20);
}

#[test]
fn read_pin_strips_trailing_whitespace() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/p");
    fs.write(&project.join(".aenv"), b"experiments\n").unwrap();
    let pin = read_pin(&fs, &project).unwrap();
    assert_eq!(pin, "experiments");
}

#[test]
fn read_pin_rejects_blank_content() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/p");
    fs.write(&project.join(".aenv"), b"   \n\n").unwrap();
    let err = read_pin(&fs, &project).expect_err("must error");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn read_pin_takes_first_non_blank_line() {
    // R-33: ".aenv file at a project root containing one namespace name
    // per line." Phase 1 supports only single-namespace pin, so we take
    // the first non-blank line and ignore the rest with a warning later.
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/p");
    fs.write(&project.join(".aenv"), b"experiments\n# comment\n")
        .unwrap();
    let pin = read_pin(&fs, &project).unwrap();
    assert_eq!(pin, "experiments");
}

#[test]
fn find_project_root_returns_self_when_pin_present() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/payments-api");
    fs.write(&project.join(".aenv"), b"experiments\n").unwrap();
    let root = find_project_root(&fs, &project).unwrap();
    assert_eq!(root, project);
}

#[test]
fn find_project_root_walks_up_to_ancestor() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/payments-api");
    fs.write(&project.join(".aenv"), b"experiments\n").unwrap();
    let nested = project.join("src/handlers");
    fs.create_dir_all(&nested).unwrap();
    let root = find_project_root(&fs, &nested).unwrap();
    assert_eq!(root, project);
}

#[test]
fn find_project_root_returns_err_when_no_ancestor_pinned() {
    let fs = MockFilesystem::new();
    let nested = PathBuf::from("/tmp/wherever/deep/path");
    fs.create_dir_all(&nested).unwrap();
    let err = find_project_root(&fs, &nested).expect_err("must error");
    assert!(matches!(err, AenvError::ProjectNotPinned));
}

#[test]
fn find_project_root_prefers_nearest_pin_ancestor() {
    // Per functional spec §9 "Nested projects": the nearest-ancestor
    // .aenv wins.
    let fs = MockFilesystem::new();
    let monorepo = PathBuf::from("/projects/monorepo");
    let inner = monorepo.join("experiments");
    fs.write(&monorepo.join(".aenv"), b"detailed-execution\n")
        .unwrap();
    fs.write(&inner.join(".aenv"), b"experiments\n").unwrap();
    let root = find_project_root(&fs, &inner).unwrap();
    assert_eq!(root, inner);
}

#[test]
fn find_project_root_skips_dot_aenv_directory() {
    // Regression: the default AENV_HOME is `$HOME/.aenv`, which is a
    // directory. If the project-root walk treats it as a pin, every
    // command run anywhere under $HOME silently resolves to $HOME — and
    // `aenv use` then tries to overwrite the registry dir with a file
    // (EISDIR). The walk must only match regular files.
    let fs = MockFilesystem::new();
    let home = PathBuf::from("/home/user");
    // Simulate AENV_HOME at $HOME/.aenv — a directory holding the registry.
    fs.create_dir_all(&home.join(".aenv/envs")).unwrap();
    let project = home.join("code/charybdis");
    fs.create_dir_all(&project).unwrap();
    // The walk from charybdis must NOT return /home/user just because
    // $HOME/.aenv exists as a directory.
    let err = find_project_root(&fs, &project).expect_err("must error");
    assert!(matches!(err, AenvError::ProjectNotPinned));
}
