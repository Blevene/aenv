use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::skills::local::resolve_local;
use std::path::PathBuf;

#[test]
fn resolves_when_skill_md_exists() {
    let fs = MockFilesystem::new();
    fs.create_dir_all(&PathBuf::from("/local/my-skill"))
        .unwrap();
    fs.write(
        &PathBuf::from("/local/my-skill/SKILL.md"),
        b"---\nname: x\n---\nbody",
    )
    .unwrap();

    let r = resolve_local(&fs, &PathBuf::from("/local/my-skill"), "my-skill", None).unwrap();
    assert_eq!(r.source_path, PathBuf::from("/local/my-skill"));
    assert!(r.resolved_ref.is_none());
    // Same bytes always produce the same hash.
    assert!(r.resolved_hash.starts_with("sha256:"));
    assert!(r.resolved_hash.len() > "sha256:".len());
}

#[test]
fn hash_changes_with_content() {
    let fs = MockFilesystem::new();
    fs.write(&PathBuf::from("/a/SKILL.md"), b"first").unwrap();
    fs.write(&PathBuf::from("/b/SKILL.md"), b"second").unwrap();
    let r1 = resolve_local(&fs, &PathBuf::from("/a"), "x", None).unwrap();
    let r2 = resolve_local(&fs, &PathBuf::from("/b"), "x", None).unwrap();
    assert_ne!(r1.resolved_hash, r2.resolved_hash);
}

#[test]
fn errors_when_skill_md_missing() {
    let fs = MockFilesystem::new();
    fs.create_dir_all(&PathBuf::from("/empty/dir")).unwrap();
    let err = resolve_local(&fs, &PathBuf::from("/empty/dir"), "x", None).unwrap_err();
    assert!(err.to_string().contains("SKILL.md"));
}

#[test]
fn errors_when_directory_missing() {
    let fs = MockFilesystem::new();
    let err = resolve_local(&fs, &PathBuf::from("/does/not/exist"), "x", None).unwrap_err();
    assert!(
        err.to_string().contains("does not exist") || err.to_string().contains("not found"),
        "msg = {err}"
    );
}
