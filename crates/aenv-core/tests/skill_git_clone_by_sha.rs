//! Regression test for the git_clone-by-SHA fix.
//!
//! Before the fix, `git_clone(url, Some(<40-char SHA>), dest)` invoked
//! `git clone --depth 1 --branch <SHA>` which git rejects with
//! "fatal: Remote branch <SHA> not found in upstream origin". This bites any
//! user who pins a skill via `aenv skill import --pin <branch>` (which writes
//! the resolved SHA into the manifest) and then activates on a fresh machine
//! whose `~/.aenv/cache/skills/` is empty.
//!
//! The fix routes SHA-shaped refs through `git init` + `git fetch --depth 1
//! origin <sha>` + `git checkout FETCH_HEAD`.

use aenv_core::skills::git::{git_available, git_clone, git_resolve_ref};
use std::process::Command;
use tempfile::tempdir;

fn skip_unless_git() -> bool {
    git_available()
}

fn make_bare_repo_with_one_commit() -> tempfile::TempDir {
    let bare = tempdir().unwrap();
    Command::new("git")
        .args(["init", "--bare"])
        .arg(bare.path())
        .status()
        .unwrap();
    let work = tempdir().unwrap();
    Command::new("git")
        .args(["clone"])
        .arg(bare.path())
        .arg(work.path())
        .status()
        .unwrap();
    std::fs::write(work.path().join("README.md"), b"first\n").unwrap();
    Command::new("git")
        .current_dir(work.path())
        .args(["add", "."])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(work.path())
        .args([
            "-c",
            "user.email=t@e",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "init",
        ])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(work.path())
        .args(["push", "origin", "HEAD:master"])
        .status()
        .unwrap();
    bare
}

#[test]
fn clone_succeeds_when_ref_is_a_full_sha() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_bare_repo_with_one_commit();
    let url = format!("file://{}", bare.path().display());

    // First resolve `master` to a real SHA — this is what `--pin master` would
    // store in the manifest.
    let sha = git_resolve_ref(&url, Some("master")).unwrap();
    assert_eq!(sha.len(), 40, "expected full SHA, got {sha:?}");
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));

    // Now clone using that SHA as the ref. Before the fix this would call
    // `git clone --branch <sha>` and fail.
    let dest = tempdir().unwrap();
    let dest_path = dest.path().join("clone");
    let resolved = git_clone(&url, Some(&sha), &dest_path).unwrap();
    assert_eq!(resolved, sha, "clone should report the same SHA");
    assert!(
        dest_path.join("README.md").exists(),
        "cloned content should be present"
    );
}

#[test]
fn clone_still_works_for_branch_name() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_bare_repo_with_one_commit();
    let url = format!("file://{}", bare.path().display());

    let dest = tempdir().unwrap();
    let dest_path = dest.path().join("clone");
    let resolved = git_clone(&url, Some("master"), &dest_path).unwrap();
    assert_eq!(resolved.len(), 40);
    assert!(dest_path.join("README.md").exists());
}
