//! End-to-end tests for `aenv global import <git-url>`.
//!
//! Two flavors:
//! - A network-dependent test against the real `juanandresgs/claude-ctrl`
//!   repo, marked `#[ignore]` so the default `cargo test` run skips it.
//! - A network-less test that creates a local bare-style git repo via
//!   `git init` + `git commit`, then imports it via a `file://` URL. This
//!   requires `git` on PATH but no network.

use std::path::{Path, PathBuf};
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn git_in(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("git invocation");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn global_import_local_git_repo_clones_and_imports() {
    if !git_available() {
        eprintln!("git not on PATH; skipping");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    // Build a tiny local git repo with a claude-ctrl-like layout.
    let repo = canon(tmp.path()).join("upstream");
    std::fs::create_dir_all(&repo).unwrap();
    git_in(&repo, &["init", "--quiet", "-b", "main"]);
    std::fs::write(repo.join("CLAUDE.md"), b"# repo-imported").unwrap();
    std::fs::create_dir_all(repo.join("agents")).unwrap();
    std::fs::write(repo.join("agents/hello.md"), b"agent hello").unwrap();
    std::fs::write(repo.join("install.sh"), b"#!/bin/sh\necho install\n").unwrap();
    git_in(&repo, &["add", "."]);
    git_in(&repo, &["commit", "--quiet", "-m", "initial"]);

    // Build a `file://` URL pointing at the repo (git allows local-file clones).
    let url = format!("file://{}", repo.display());

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "import", &url, "local-repo"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git import failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let ns_dir = aenv_home.join("envs/local-repo");
    assert!(ns_dir.join("aenv.toml").exists());
    assert!(ns_dir.join("user/.claude/CLAUDE.md").exists());
    assert_eq!(
        std::fs::read(ns_dir.join("user/.claude/CLAUDE.md")).unwrap(),
        b"# repo-imported"
    );
    assert!(ns_dir.join("user/.claude/agents/hello.md").exists());
    assert!(ns_dir.join("install.sh").exists());
}

#[test]
fn global_import_local_git_repo_with_pin_checks_out_ref() {
    if !git_available() {
        eprintln!("git not on PATH; skipping");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let repo = canon(tmp.path()).join("pinned-upstream");
    std::fs::create_dir_all(&repo).unwrap();
    git_in(&repo, &["init", "--quiet", "-b", "main"]);
    std::fs::write(repo.join("CLAUDE.md"), b"v1").unwrap();
    git_in(&repo, &["add", "."]);
    git_in(&repo, &["commit", "--quiet", "-m", "v1"]);
    git_in(&repo, &["tag", "v1"]);
    std::fs::write(repo.join("CLAUDE.md"), b"v2").unwrap();
    git_in(&repo, &["add", "."]);
    git_in(&repo, &["commit", "--quiet", "-m", "v2"]);

    let url = format!("file://{}", repo.display());

    // Pin to v1 — should see "v1" bytes, not "v2".
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "import", &url, "pinned", "--pin", "v1"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "pinned import failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let body = std::fs::read(aenv_home.join("envs/pinned/user/.claude/CLAUDE.md")).unwrap();
    assert_eq!(body, b"v1", "expected the pinned-v1 content");
}

#[test]
fn global_import_git_default_name_from_url() {
    if !git_available() {
        eprintln!("git not on PATH; skipping");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let repo = canon(tmp.path()).join("auto-named");
    std::fs::create_dir_all(&repo).unwrap();
    git_in(&repo, &["init", "--quiet", "-b", "main"]);
    std::fs::write(repo.join("CLAUDE.md"), b"x").unwrap();
    git_in(&repo, &["add", "."]);
    git_in(&repo, &["commit", "--quiet", "-m", "initial"]);
    let url = format!("file://{}", repo.display());

    // No name argument — should default to the URL's last segment.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "import", &url])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "import failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(aenv_home.join("envs/auto-named/aenv.toml").exists());
}

#[test]
#[ignore = "requires network; run locally before release"]
fn global_import_git_url_clones_and_imports() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args([
            "global",
            "import",
            "https://github.com/juanandresgs/claude-ctrl",
            "claude-cntrl",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "network import failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let ns_dir = aenv_home.join("envs/claude-cntrl");
    assert!(ns_dir.join("aenv.toml").exists());
}
