use aenv_core::skills::git::{git_available, git_clone, git_resolve_ref};
use std::process::Command;
use tempfile::tempdir;

fn skip_unless_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn git_available_returns_true_when_on_path() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    assert!(git_available());
}

#[test]
fn ls_remote_resolves_local_bare_repo_head() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = tempdir().unwrap();
    let bare_path = bare.path();
    // Initialize a bare repo with one commit.
    Command::new("git")
        .args(["init", "--bare"])
        .arg(bare_path)
        .status()
        .unwrap();
    let work = tempdir().unwrap();
    let work_path = work.path();
    Command::new("git")
        .args(["clone"])
        .arg(bare_path)
        .arg(work_path)
        .status()
        .unwrap();
    std::fs::write(work_path.join("README.md"), b"hi").unwrap();
    Command::new("git").current_dir(work_path).args(["add", "."]).status().unwrap();
    Command::new("git")
        .current_dir(work_path)
        .args([
            "-c", "user.email=t@e", "-c", "user.name=t",
            "commit", "-m", "init",
        ])
        .status()
        .unwrap();
    Command::new("git").current_dir(work_path).args(["push", "origin", "HEAD:master"]).status().unwrap();

    let url = format!("file://{}", bare_path.display());
    let sha = git_resolve_ref(&url, None).unwrap();
    assert_eq!(sha.len(), 40, "expected full SHA, got {sha:?}");
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn clone_to_destination_returns_resolved_sha() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    // Set up a tiny bare repo as in the ls_remote test.
    let bare = tempdir().unwrap();
    let bare_path = bare.path();
    Command::new("git").args(["init", "--bare"]).arg(bare_path).status().unwrap();
    let work = tempdir().unwrap();
    let work_path = work.path();
    Command::new("git").args(["clone"]).arg(bare_path).arg(work_path).status().unwrap();
    std::fs::write(work_path.join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    Command::new("git").current_dir(work_path).args(["add", "."]).status().unwrap();
    Command::new("git")
        .current_dir(work_path)
        .args(["-c", "user.email=t@e", "-c", "user.name=t", "commit", "-m", "init"])
        .status()
        .unwrap();
    Command::new("git").current_dir(work_path).args(["push", "origin", "HEAD:master"]).status().unwrap();

    let url = format!("file://{}", bare_path.display());
    let dest = tempdir().unwrap();
    let sha = git_clone(&url, None, dest.path()).unwrap();
    assert_eq!(sha.len(), 40);
    assert!(dest.path().join("SKILL.md").exists());
}
