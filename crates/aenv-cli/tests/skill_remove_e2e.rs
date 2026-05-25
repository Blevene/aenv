//! E2E tests for `aenv skill remove` and `aenv cache prune`.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

fn run(aenv_home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .args(args)
        .env("AENV_HOME", aenv_home)
        .output()
        .expect("aenv ran")
}

fn fresh_home_with_namespace(adapter: &str, ns: &str) -> TempDir {
    let home = TempDir::new().unwrap();
    let out = run(home.path(), &["create", ns, "--adapter", adapter]);
    assert!(
        out.status.success(),
        "create failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    home
}

#[test]
fn skill_remove_authored_drops_manifest_entry_and_files() {
    let home = fresh_home_with_namespace("claude-code", "my-style");
    // Author a skill, then remove it.
    let out = run(
        home.path(),
        &["skill", "new", "my-skill", "--ns", "my-style"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let skill_dir = home.path().join("envs/my-style/.claude/skills/my-skill");
    assert!(skill_dir.exists(), "skill dir should exist before remove");

    let out = run(
        home.path(),
        &["skill", "remove", "my-skill", "--ns", "my-style"],
    );
    assert!(
        out.status.success(),
        "remove failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Removed skill 'my-skill'"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("authored"), "should report skill flavor");

    // Skill dir gone.
    assert!(!skill_dir.exists(), "skill dir should be removed");

    // Manifest no longer mentions it.
    let manifest = std::fs::read_to_string(home.path().join("envs/my-style/aenv.toml")).unwrap();
    assert!(
        !manifest.contains("my-skill"),
        "manifest still references skill:\n{manifest}"
    );
}

#[test]
fn skill_remove_imported_keeps_cache_intact() {
    let home = fresh_home_with_namespace("claude-code", "my-style");
    // Set up a fake local source so we don't need network.
    let src = TempDir::new().unwrap();
    let skill_dir = src.path().join("scientific-skills/example");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        b"---\nname: example\ndescription: x\n---\n",
    )
    .unwrap();

    let out = run(
        home.path(),
        &[
            "skill",
            "import",
            &src.path().display().to_string(),
            "--ns",
            "my-style",
            "--path",
            "scientific-skills/example",
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run(
        home.path(),
        &["skill", "remove", "example", "--ns", "my-style"],
    );
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("imported"));
    assert!(
        stdout.contains("cache prune"),
        "should point at cache prune; stdout: {stdout}"
    );
    // The source dir we set up shouldn't have been touched.
    assert!(skill_dir.join("SKILL.md").exists());
}

#[test]
fn skill_remove_errors_on_unknown_skill() {
    let home = fresh_home_with_namespace("claude-code", "my-style");
    let out = run(
        home.path(),
        &["skill", "remove", "ghost", "--ns", "my-style"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no skill named 'ghost'"));
}

#[test]
fn cache_prune_removes_dirs_no_skill_references() {
    let home = fresh_home_with_namespace("claude-code", "my-style");
    // Plant two cache dirs by hand — one referenced, one orphaned.
    let cache_root = home.path().join("cache/skills");

    // Compute the source-hash for our fake source the same way aenv would.
    // Easier path: import a real skill (this populates cache via aenv's own
    // source_hash) then plant a second, orphaned cache dir under a fake hash.
    let src = TempDir::new().unwrap();
    let skill_dir = src.path().join("s/example");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        b"---\nname: example\ndescription: x\n---\n",
    )
    .unwrap();
    run(
        home.path(),
        &[
            "skill",
            "import",
            &src.path().display().to_string(),
            "--ns",
            "my-style",
            "--path",
            "s/example",
        ],
    );
    // Local-source imports don't populate cache (resolve_local has no clone
    // step), so we instead plant orphan cache dirs directly.
    let orphan_hash_dir = cache_root.join("deadbeefdeadbeef");
    std::fs::create_dir_all(orphan_hash_dir.join("v1.0")).unwrap();
    std::fs::write(orphan_hash_dir.join("v1.0/SKILL.md"), b"orphan").unwrap();

    let out = run(home.path(), &["cache", "prune"]);
    assert!(
        out.status.success(),
        "prune failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Pruned 1"), "stdout: {stdout}");
    assert!(
        !orphan_hash_dir.exists(),
        "orphan dir should be removed (parent pruned because empty)"
    );
}

#[test]
fn cache_prune_handles_empty_cache() {
    let home = TempDir::new().unwrap();
    // Bootstrap ~/.aenv but don't populate cache.
    run(home.path(), &["list"]);
    let out = run(home.path(), &["cache", "prune"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Cache empty") || stdout.contains("Pruned 0"),
        "stdout: {stdout}"
    );
}
