//! End-to-end CLI integration test.
//!
//! Drives the built `aenv` binary as a subprocess against a real
//! `tempfile::tempdir`. Exercises the full happy path: create -> use ->
//! activate -> status -> deactivate -> restore.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: PathBuf,
    project: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
        // Canonicalize for macOS where /var is a symlink to /private/var —
        // tempdir().path() returns /var/folders/..., but `realpath` and
        // `read_link` return /private/var/folders/... . Use canonical paths
        // everywhere so equality assertions hold.
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        let project = std::fs::canonicalize(project_guard.path()).unwrap();
        Self {
            _aenv_home_guard: aenv_home_guard,
            _project_guard: project_guard,
            aenv_home,
            project,
        }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new(bin());
        c.env("AENV_HOME", &self.aenv_home);
        c
    }

    fn aenv_home(&self) -> &Path {
        &self.aenv_home
    }

    fn project(&self) -> &Path {
        &self.project
    }
}

fn assert_success(out: &std::process::Output, ctx: &str) {
    if !out.status.success() {
        panic!(
            "{ctx} failed: status={:?}, stdout={}, stderr={}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn happy_path_create_use_activate_status_deactivate() {
    let h = Harness::new();

    // 1. Create a namespace.
    let out = h.cmd().args(["create", "experiments"]).output().unwrap();
    assert_success(&out, "create");

    // 2. Author a CLAUDE.md in the namespace.
    let ns_dir = h.aenv_home().join("envs/experiments");
    let ns_claude = ns_dir.join("CLAUDE.md");
    std::fs::write(&ns_claude, b"namespace disposition\n").unwrap();
    // Edit the manifest to register the claude-code adapter for CLAUDE.md.
    std::fs::write(
        ns_dir.join("aenv.toml"),
        b"name = \"experiments\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    // 3. Pin the project.
    let out = h
        .cmd()
        .args(["use", "experiments", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use");
    assert_eq!(
        std::fs::read_to_string(h.project().join(".aenv")).unwrap().trim(),
        "experiments"
    );

    // 4. Activate.
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "activate");
    let project_claude = h.project().join("CLAUDE.md");
    let meta = std::fs::symlink_metadata(&project_claude).unwrap();
    assert!(meta.file_type().is_symlink(), "expected symlink");
    let target = std::fs::read_link(&project_claude).unwrap();
    assert_eq!(target, ns_claude);

    // 5. Status reports the active namespace.
    let out = h
        .cmd()
        .args(["status", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "status");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("experiments"));
    assert!(stdout.contains("CLAUDE.md"));

    // 6. Deactivate. The symlink goes away; no backup to restore (no original).
    let out = h
        .cmd()
        .args(["deactivate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "deactivate");
    assert!(!project_claude.exists());
    assert!(!h.project().join(".aenv-state/state.json").exists());
}

#[test]
fn backup_then_restore_round_trip() {
    let h = Harness::new();

    // Pre-populate project with a user CLAUDE.md.
    let project_claude = h.project().join("CLAUDE.md");
    std::fs::write(&project_claude, b"user-authored\n").unwrap();

    // Create + populate namespace.
    let out = h.cmd().args(["create", "experiments"]).output().unwrap();
    assert_success(&out, "create");
    let ns_dir = h.aenv_home().join("envs/experiments");
    std::fs::write(ns_dir.join("CLAUDE.md"), b"namespace\n").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        b"name = \"experiments\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    // Activate -> user file backed up; symlink installed.
    let out = h
        .cmd()
        .args(["use", "experiments", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use");
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "activate");
    assert!(
        std::fs::symlink_metadata(&project_claude)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    // Deactivate -> backup is restored.
    let out = h
        .cmd()
        .args(["deactivate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "deactivate");
    let restored = std::fs::read_to_string(&project_claude).unwrap();
    assert_eq!(restored, "user-authored\n");
    // Symlink-bit gone.
    assert!(
        !std::fs::symlink_metadata(&project_claude)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn list_after_create_shows_namespace() {
    let h = Harness::new();
    h.cmd().args(["create", "a"]).output().unwrap();
    h.cmd().args(["create", "b"]).output().unwrap();
    let out = h.cmd().args(["list"]).output().unwrap();
    assert_success(&out, "list");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("a"));
    assert!(stdout.contains("b"));
}

#[test]
fn create_then_delete_round_trip() {
    let h = Harness::new();
    h.cmd().args(["create", "kill-me"]).output().unwrap();
    let out = h.cmd().args(["delete", "kill-me"]).output().unwrap();
    assert_success(&out, "delete");
    assert!(!h.aenv_home().join("envs/kill-me").exists());
}

#[test]
fn activate_unknown_namespace_exits_ten() {
    let h = Harness::new();
    // Pin to a non-existent namespace.
    std::fs::write(h.project().join(".aenv"), b"nope\n").unwrap();
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(10));
}

#[test]
fn deactivate_without_active_state_exits_thirteen() {
    // Missing state.json -> ActivationConflict (exit 13), not
    // ProjectNotPinned (exit 20). The latter is reserved for missing
    // .aenv pin file specifically.
    let h = Harness::new();
    let out = h
        .cmd()
        .args(["deactivate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(13));
}

#[test]
fn activate_never_writes_outside_adapter_declared_paths() {
    // PRD R-60 invariant: aenv shall never modify a project file outside
    // the paths declared by active adapters. Activate, then enumerate
    // every regular file or symlink under the project root and assert
    // each one is either in the adapter's `files` set or under `.aenv/`.
    let h = Harness::new();

    // Setup: namespace ships CLAUDE.md only.
    let out = h.cmd().args(["create", "experiments"]).output().unwrap();
    assert_success(&out, "create");
    let ns_dir = h.aenv_home().join("envs/experiments");
    std::fs::write(ns_dir.join("CLAUDE.md"), b"x").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        b"name = \"experiments\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let out = h
        .cmd()
        .args(["use", "experiments", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use");
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "activate");

    // Walk the project tree; every entry must be the .aenv pin file, under
    // .aenv/, or in the adapter's declared files set.
    let declared: std::collections::HashSet<PathBuf> =
        [PathBuf::from(".aenv"), PathBuf::from("CLAUDE.md")]
            .into_iter()
            .collect();
    walk_assert_only_declared(h.project(), h.project(), &declared);
}

fn walk_assert_only_declared(
    root: &Path,
    current: &Path,
    declared: &std::collections::HashSet<PathBuf>,
) {
    for entry in std::fs::read_dir(current).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap().to_path_buf();
        // .aenv-state/ subtree (state, backups) is aenv's own — skip.
        if rel.starts_with(".aenv-state") {
            continue;
        }
        // Recurse into directories.
        let meta = std::fs::symlink_metadata(&path).unwrap();
        if meta.file_type().is_dir() {
            walk_assert_only_declared(root, &path, declared);
            continue;
        }
        // Files and symlinks must be in the declared set.
        assert!(
            declared.contains(&rel),
            "R-60 violation: project has un-declared file {rel:?}",
        );
    }
}
