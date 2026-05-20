//! Tests for `restore_latest_backup`.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::restore::restore_latest_backup;
use aenv_core::AenvError;
use std::path::PathBuf;

#[test]
fn restores_latest_backup_set() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");

    // Two backup sets; latest by lex order wins (epoch timestamps sort lex).
    fs.write(
        &project.join(".aenv-state/backup/epoch-1000/CLAUDE.md"),
        b"older",
    )
    .unwrap();
    fs.write(
        &project.join(".aenv-state/backup/epoch-2000/CLAUDE.md"),
        b"newer",
    )
    .unwrap();
    fs.write(&project.join("CLAUDE.md"), b"current symlink target")
        .unwrap();

    restore_latest_backup(&fs, &project).unwrap();

    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"newer");
}

#[test]
fn restores_multiple_files_in_one_set() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.write(
        &project.join(".aenv-state/backup/epoch-1000/CLAUDE.md"),
        b"a",
    )
    .unwrap();
    fs.write(
        &project.join(".aenv-state/backup/epoch-1000/.claude/foo.md"),
        b"b",
    )
    .unwrap();

    restore_latest_backup(&fs, &project).unwrap();

    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"a");
    assert_eq!(fs.read(&project.join(".claude/foo.md")).unwrap(), b"b");
}

#[test]
fn errors_when_no_backups_exist() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = restore_latest_backup(&fs, &project).expect_err("must error");
    assert!(matches!(err, AenvError::ActivationConflict(_)));
}

#[test]
fn errors_when_aenv_dir_missing() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = restore_latest_backup(&fs, &project).expect_err("must error");
    assert!(matches!(err, AenvError::ActivationConflict(_)));
}

#[test]
fn restore_is_idempotent_re_running_reproduces_state() {
    // The doc comment promises: "the backup directory is left intact so
    // the same backup set can be restored repeatedly." Lock that promise.
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.write(
        &project.join(".aenv-state/backup/epoch-1000/CLAUDE.md"),
        b"original",
    )
    .unwrap();

    restore_latest_backup(&fs, &project).unwrap();
    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"original");

    // User edits the restored file.
    fs.write(&project.join("CLAUDE.md"), b"edited").unwrap();

    // Second restore overwrites the edit with the backup contents again.
    restore_latest_backup(&fs, &project).unwrap();
    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"original");

    // Backup file is still there (not consumed).
    assert_eq!(
        fs.read(&project.join(".aenv-state/backup/epoch-1000/CLAUDE.md"))
            .unwrap(),
        b"original"
    );
}
