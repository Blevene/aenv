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
    let fs = MockFilesystem::new();
    fs.write(&p("/a/b/c.txt"), b"hello").unwrap();
    assert_eq!(fs.read(&p("/a/b/c.txt")).unwrap(), b"hello");
}

#[test]
fn write_auto_creates_parent_dirs() {
    let fs = MockFilesystem::new();
    fs.write(&p("/a/b/c.txt"), b"x").unwrap();
    let meta = fs.metadata(&p("/a/b")).unwrap();
    assert_eq!(meta.kind, FileKind::Directory);
}

#[test]
fn rename_moves_file() {
    let fs = MockFilesystem::new();
    fs.write(&p("/from"), b"data").unwrap();
    fs.rename(&p("/from"), &p("/to")).unwrap();
    assert!(!fs.exists(&p("/from")).unwrap());
    assert_eq!(fs.read(&p("/to")).unwrap(), b"data");
}

#[test]
fn remove_file_deletes() {
    let fs = MockFilesystem::new();
    fs.write(&p("/x"), b"x").unwrap();
    fs.remove_file(&p("/x")).unwrap();
    assert!(!fs.exists(&p("/x")).unwrap());
}

#[test]
fn remove_dir_all_deletes_tree() {
    let fs = MockFilesystem::new();
    fs.write(&p("/a/b/c"), b"x").unwrap();
    fs.write(&p("/a/d"), b"y").unwrap();
    fs.remove_dir_all(&p("/a")).unwrap();
    assert!(!fs.exists(&p("/a")).unwrap());
    assert!(!fs.exists(&p("/a/b/c")).unwrap());
}

#[test]
fn symlink_records_target() {
    let fs = MockFilesystem::new();
    fs.write(&p("/target"), b"t").unwrap();
    fs.symlink(&p("/target"), &p("/link")).unwrap();
    assert!(fs.is_symlink(&p("/link")).unwrap());
    assert_eq!(fs.read_link(&p("/link")).unwrap(), p("/target"));
}

#[test]
fn read_follows_symlink() {
    let fs = MockFilesystem::new();
    fs.write(&p("/target"), b"t").unwrap();
    fs.symlink(&p("/target"), &p("/link")).unwrap();
    assert_eq!(fs.read(&p("/link")).unwrap(), b"t");
}

#[test]
fn symlink_metadata_reports_symlink_kind_not_target() {
    let fs = MockFilesystem::new();
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
    let fs = MockFilesystem::new();
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
    let fs = MockFilesystem::new();
    fs.fail_writes_to(&p("/cursed"));
    let result = fs.write(&p("/cursed"), b"x");
    assert!(result.is_err(), "expected injected failure");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Other);
}

// ---- Contract-divergence regression tests (Phase 0.5 batch 2) ----

#[test]
fn rename_moves_descendants_with_directory() {
    // Real std::fs::rename of a directory moves its whole subtree. The mock
    // must rebase every descendant path or Phase 1's staging-rename pattern
    // will produce ghost keys.
    let fs = MockFilesystem::new();
    fs.write(&p("/src/a/leaf"), b"x").unwrap();
    fs.write(&p("/src/b"), b"y").unwrap();
    fs.create_dir_all(&p("/src/c")).unwrap();

    fs.rename(&p("/src"), &p("/dst")).unwrap();

    // Old paths gone.
    assert!(!fs.exists(&p("/src")).unwrap());
    assert!(!fs.exists(&p("/src/a/leaf")).unwrap());
    assert!(!fs.exists(&p("/src/a")).unwrap());
    assert!(!fs.exists(&p("/src/b")).unwrap());
    assert!(!fs.exists(&p("/src/c")).unwrap());

    // New paths present, contents preserved.
    assert_eq!(fs.read(&p("/dst/a/leaf")).unwrap(), b"x");
    assert_eq!(fs.read(&p("/dst/b")).unwrap(), b"y");
    assert_eq!(fs.metadata(&p("/dst/c")).unwrap().kind, FileKind::Directory);
}

#[test]
fn write_over_existing_directory_errors() {
    // Real std::fs::write fails when path is currently a directory.
    let fs = MockFilesystem::new();
    fs.create_dir_all(&p("/dir")).unwrap();
    let err = fs.write(&p("/dir"), b"x").expect_err("must error");
    assert!(err.to_string().contains("is a directory"), "got: {err}");
}

#[test]
fn remove_dir_all_on_file_errors() {
    // Real std::fs::remove_dir_all errors when target is a regular file.
    let fs = MockFilesystem::new();
    fs.write(&p("/file"), b"x").unwrap();
    let err = fs.remove_dir_all(&p("/file")).expect_err("must error");
    assert!(err.to_string().contains("not a directory"), "got: {err}");
}

#[test]
fn list_dir_distinguishes_missing_from_not_a_directory() {
    let fs = MockFilesystem::new();
    fs.write(&p("/file"), b"x").unwrap();

    let missing = fs.list_dir(&p("/nope")).expect_err("must error");
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);

    let not_a_dir = fs.list_dir(&p("/file")).expect_err("must error");
    assert!(
        not_a_dir.to_string().contains("not a directory"),
        "got: {not_a_dir}"
    );
}

#[test]
fn fail_stats_on_makes_exists_return_err() {
    // The whole point of Filesystem::exists returning io::Result<bool> (not
    // bool) is so permission errors during stat surface to callers. The
    // mock needs a way to exercise that Err branch.
    let fs = MockFilesystem::new();
    fs.write(&p("/locked"), b"x").unwrap();
    fs.fail_stats_on(&p("/locked"));

    let err = fs.exists(&p("/locked")).expect_err("must error");
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);

    // Other stat-shaped methods are also affected.
    assert_eq!(
        fs.metadata(&p("/locked")).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        fs.symlink_metadata(&p("/locked")).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        fs.is_symlink(&p("/locked")).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

#[test]
fn relative_symlink_target_resolves_against_link_parent() {
    // POSIX semantics: a symlink with a relative target resolves the target
    // against the link's parent directory, not the process cwd. The mock
    // honors this so Phase 1 tests that use relative paths behave like real.
    let fs = MockFilesystem::new();
    fs.write(&p("/a/target"), b"hit").unwrap();
    // /a/link -> "target" (relative) should resolve to /a/target.
    fs.symlink(&p("target"), &p("/a/link")).unwrap();
    assert_eq!(fs.read(&p("/a/link")).unwrap(), b"hit");
}

#[test]
fn phase_1_shaped_scenario_backup_then_restore() {
    // End-to-end exercise of the operations Phase 1 activation will use,
    // composed against the mock. This is the test that would have caught
    // the rename/write/remove_dir_all contract gaps if it had existed
    // during Phase 0.

    let fs = MockFilesystem::new();

    // Initial state: project has a user CLAUDE.md; namespace has its own.
    fs.write(&p("/project/CLAUDE.md"), b"user content").unwrap();
    fs.write(&p("/registry/ns/CLAUDE.md"), b"namespace content")
        .unwrap();

    // Activation step 1: back up the project file.
    fs.create_dir_all(&p("/project/.aenv-state/backup/2026-05-20"))
        .unwrap();
    fs.rename(
        &p("/project/CLAUDE.md"),
        &p("/project/.aenv-state/backup/2026-05-20/CLAUDE.md"),
    )
    .unwrap();
    assert!(!fs.exists(&p("/project/CLAUDE.md")).unwrap());
    assert_eq!(
        fs.read(&p("/project/.aenv-state/backup/2026-05-20/CLAUDE.md"))
            .unwrap(),
        b"user content"
    );

    // Activation step 2: symlink the namespace file into place.
    fs.symlink(&p("/registry/ns/CLAUDE.md"), &p("/project/CLAUDE.md"))
        .unwrap();
    assert!(fs.is_symlink(&p("/project/CLAUDE.md")).unwrap());
    // Reading through the symlink returns the namespace content.
    assert_eq!(
        fs.read(&p("/project/CLAUDE.md")).unwrap(),
        b"namespace content"
    );
    // symlink_metadata sees the link itself, not the target.
    assert_eq!(
        fs.symlink_metadata(&p("/project/CLAUDE.md")).unwrap().kind,
        FileKind::Symlink
    );

    // Deactivation step 1: remove the symlink.
    fs.remove_file(&p("/project/CLAUDE.md")).unwrap();
    assert!(!fs.exists(&p("/project/CLAUDE.md")).unwrap());
    // Namespace file untouched.
    assert_eq!(
        fs.read(&p("/registry/ns/CLAUDE.md")).unwrap(),
        b"namespace content"
    );

    // Deactivation step 2: restore the backup.
    fs.rename(
        &p("/project/.aenv-state/backup/2026-05-20/CLAUDE.md"),
        &p("/project/CLAUDE.md"),
    )
    .unwrap();
    assert_eq!(fs.read(&p("/project/CLAUDE.md")).unwrap(), b"user content");
    // symlink_metadata now reports File (the restored regular file).
    assert_eq!(
        fs.symlink_metadata(&p("/project/CLAUDE.md")).unwrap().kind,
        FileKind::File
    );
}
