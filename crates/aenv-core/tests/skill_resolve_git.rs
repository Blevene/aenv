use aenv_core::fs::RealFilesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::skills::git::git_available;
use aenv_core::skills::git_source::resolve_git;
use std::process::Command;
use tempfile::tempdir;

fn skip_unless_git() -> bool {
    git_available()
}

fn make_bare_repo_with_skill() -> tempfile::TempDir {
    let bare = tempdir().unwrap();
    let bare_path = bare.path();
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
    std::fs::create_dir_all(work_path.join("dummy-skill")).unwrap();
    std::fs::write(
        work_path.join("dummy-skill/SKILL.md"),
        b"---\nname: dummy-skill\ndescription: a test skill\n---\nbody\n",
    )
    .unwrap();
    Command::new("git")
        .current_dir(work_path)
        .args(["add", "."])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(work_path)
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
        .current_dir(work_path)
        .args(["push", "origin", "HEAD:master"])
        .status()
        .unwrap();
    bare
}

#[test]
fn resolves_git_source_to_cache_directory() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_bare_repo_with_skill();
    let aenv_home = tempdir().unwrap();
    let layout = RegistryLayout::new(aenv_home.path().to_path_buf());
    let url = format!("file://{}", bare.path().display());

    let fs = RealFilesystem;
    let result = resolve_git(&fs, &layout, &url, Some("master"), "dummy-skill").unwrap();
    assert!(result.source_path.exists());
    assert!(result.source_path.join("dummy-skill/SKILL.md").exists());
    assert_eq!(result.resolved_ref.as_deref().map(|s| s.len()), Some(40));
    assert!(result.resolved_hash.starts_with("sha256:"));
}

#[test]
fn second_resolution_uses_cache() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_bare_repo_with_skill();
    let aenv_home = tempdir().unwrap();
    let layout = RegistryLayout::new(aenv_home.path().to_path_buf());
    let url = format!("file://{}", bare.path().display());
    let fs = RealFilesystem;

    let r1 = resolve_git(&fs, &layout, &url, Some("master"), "dummy-skill").unwrap();
    let r2 = resolve_git(&fs, &layout, &url, Some("master"), "dummy-skill").unwrap();
    assert_eq!(r1.source_path, r2.source_path);
    assert_eq!(r1.resolved_ref, r2.resolved_ref);
}

#[test]
fn unreachable_url_returns_remote_unreachable() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let aenv_home = tempdir().unwrap();
    let layout = RegistryLayout::new(aenv_home.path().to_path_buf());
    let fs = RealFilesystem;
    let err = resolve_git(&fs, &layout, "file:///definitely/not/a/repo", None, "x").unwrap_err();
    assert_eq!(err.exit_code(), 14);
}
