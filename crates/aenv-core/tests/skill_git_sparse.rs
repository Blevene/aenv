//! Sparse-checkout for `--path` skill imports.
//!
//! When a skill is imported from a monorepo subdir, `git_clone(.., Some(path))`
//! must materialize only that subtree (cone-mode sparse checkout), and
//! `ensure_sparse_path` must add further subtrees to an existing sparse clone
//! so multiple skills from the same repo+ref accumulate without re-cloning.
//! This keeps one tiny skill out of a huge repo from pulling the whole tree.

use aenv_core::skills::git::{ensure_sparse_path, git_available, git_clone, git_resolve_ref};
use std::process::Command;
use tempfile::tempdir;

/// Bare repo on `master` with three top-level skill dirs: a/, b/, c/.
fn make_repo_with_three_dirs() -> tempfile::TempDir {
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
    for d in ["a", "b", "c"] {
        std::fs::create_dir_all(work.path().join(d)).unwrap();
        std::fs::write(
            work.path().join(d).join("SKILL.md"),
            format!("---\nname: {d}\n---\n").as_bytes(),
        )
        .unwrap();
    }
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
fn sparse_clone_materializes_only_the_subpath() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_repo_with_three_dirs();
    let url = format!("file://{}", bare.path().display());
    let dest = tempdir().unwrap();
    let clone = dest.path().join("c1");

    git_clone(&url, Some("master"), &clone, Some("b")).unwrap();
    assert!(
        clone.join("b/SKILL.md").exists(),
        "requested subpath 'b' missing"
    );
    assert!(
        !clone.join("a/SKILL.md").exists(),
        "'a' should be sparse-excluded"
    );
    assert!(
        !clone.join("c/SKILL.md").exists(),
        "'c' should be sparse-excluded"
    );

    // A second skill from the same clone accumulates via the cone.
    ensure_sparse_path(&clone, "c").unwrap();
    assert!(
        clone.join("c/SKILL.md").exists(),
        "ensure_sparse_path should add 'c'"
    );
    assert!(
        !clone.join("a/SKILL.md").exists(),
        "'a' must remain excluded"
    );
}

#[test]
fn sparse_clone_by_sha_materializes_only_the_subpath() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_repo_with_three_dirs();
    let url = format!("file://{}", bare.path().display());
    let sha = git_resolve_ref(&url, Some("master")).unwrap();
    let dest = tempdir().unwrap();
    let clone = dest.path().join("c1");

    git_clone(&url, Some(&sha), &clone, Some("b")).unwrap();
    assert!(
        clone.join("b/SKILL.md").exists(),
        "requested subpath 'b' missing (sha path)"
    );
    assert!(
        !clone.join("a/SKILL.md").exists(),
        "'a' should be sparse-excluded (sha path)"
    );
}

#[test]
fn full_clone_without_subpath_materializes_everything() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_repo_with_three_dirs();
    let url = format!("file://{}", bare.path().display());
    let dest = tempdir().unwrap();
    let clone = dest.path().join("c1");

    git_clone(&url, Some("master"), &clone, None).unwrap();
    for d in ["a", "b", "c"] {
        assert!(
            clone.join(d).join("SKILL.md").exists(),
            "full clone missing {d}"
        );
    }
    // A full (non-sparse) clone must be left untouched by ensure_sparse_path.
    ensure_sparse_path(&clone, "b").unwrap();
    assert!(
        clone.join("a/SKILL.md").exists(),
        "ensure_sparse_path must not sparse-ify a full clone"
    );
}
