//! End-to-end composition test. Builds and drives the `aenv` binary against
//! a real tempdir; exercises a two-namespace chain end-to-end.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;

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
        // Canonicalize for macOS where /var is a symlink to /private/var.
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
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
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

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn write_file(dir: &Path, rel: &str, body: &[u8]) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

// ---------------------------------------------------------------------------
// Test 1: two-namespace chain section-merges CLAUDE.md
// ---------------------------------------------------------------------------

#[test]
fn composition_happy_path_two_namespace_chain_section_merges_claude_md() {
    let h = Harness::new();

    // Create base and leaf namespaces.
    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");
    let out = h.cmd().args(["create", "leaf"]).output().unwrap();
    assert_success(&out, "create leaf");

    let home = h.aenv_home();

    // Populate namespace files.
    write_file(
        &home.join("envs/base"),
        "CLAUDE.md",
        b"# Build & Test\n\ncargo test\n",
    );
    write_file(
        &home.join("envs/leaf"),
        "CLAUDE.md",
        b"# Disposition\n\nbe terse\n",
    );

    // Write manifests. Both namespaces declare claude-code adapter for CLAUDE.md.
    // The claude-code built-in adapter marks CLAUDE.md role = "instructions" which
    // triggers SectionMerge when both namespaces contribute the same path.
    std::fs::write(
        home.join("envs/base/aenv.toml"),
        b"name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(
        home.join("envs/leaf/aenv.toml"),
        b"name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    // Pin the project to leaf.
    let out = h
        .cmd()
        .args(["use", "leaf", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use leaf");

    // Activate.
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "activate");

    // CLAUDE.md must be a regular file (not a symlink) because it was
    // section-merged from two contributors.
    let claude = h.project().join("CLAUDE.md");
    let meta = std::fs::symlink_metadata(&claude).unwrap();
    assert!(
        !meta.file_type().is_symlink(),
        "section-merged CLAUDE.md must be a regular file, not a symlink"
    );

    let body = std::fs::read_to_string(&claude).unwrap();
    assert!(
        body.contains("# Build & Test"),
        "missing base section heading"
    );
    assert!(body.contains("cargo test"), "missing base content");
    assert!(
        body.contains("# Disposition"),
        "missing leaf section heading"
    );
    assert!(body.contains("be terse"), "missing leaf content");

    // `which CLAUDE.md` should report section-merge and both contributors.
    let out = h
        .cmd()
        .args(["which", "CLAUDE.md", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "which CLAUDE.md");
    let ws = stdout(&out);
    assert!(
        ws.contains("section-merge"),
        "which: expected strategy 'section-merge'"
    );
    assert!(
        ws.contains("base::CLAUDE.md"),
        "which: missing base contributor"
    );
    assert!(
        ws.contains("leaf::CLAUDE.md"),
        "which: missing leaf contributor"
    );

    // `status` must show the resolution chain and describe the merge.
    let out = h
        .cmd()
        .args(["status", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "status");
    let ss = stdout(&out);
    assert!(
        ss.contains("Resolution:       base \u{2192} leaf"),
        "status: expected 'Resolution:       base → leaf', got: {ss:?}"
    );
    assert!(
        ss.contains("merged from base + leaf"),
        "status: expected 'merged from base + leaf', got: {ss:?}"
    );

    // Deactivate: merged regular file should be removed (no backup to restore).
    let out = h
        .cmd()
        .args(["deactivate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "deactivate");
    assert!(
        !h.project().join("CLAUDE.md").exists(),
        "CLAUDE.md should be gone after deactivate"
    );
}

// ---------------------------------------------------------------------------
// Test 2: shadowed skill resolves to leaf; shadow chain recorded
// ---------------------------------------------------------------------------

#[test]
fn shadowed_skill_resolves_to_leaf_and_records_shadow() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");
    let out = h.cmd().args(["create", "leaf"]).output().unwrap();
    assert_success(&out, "create leaf");

    let home = h.aenv_home();

    // Both namespaces ship the same skill path -> leaf shadows base.
    write_file(
        &home.join("envs/base"),
        ".claude/skills/write-tests/SKILL.md",
        b"base impl",
    );
    write_file(
        &home.join("envs/leaf"),
        ".claude/skills/write-tests/SKILL.md",
        b"leaf impl",
    );

    std::fs::write(
        home.join("envs/base/aenv.toml"),
        b"name = \"base\"\n[adapters.claude-code]\nfiles = [\".claude/skills/write-tests/SKILL.md\"]\n",
    )
    .unwrap();
    std::fs::write(
        home.join("envs/leaf/aenv.toml"),
        b"name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\".claude/skills/write-tests/SKILL.md\"]\n",
    )
    .unwrap();

    let out = h
        .cmd()
        .args(["use", "leaf", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use leaf");
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "activate");

    // Leaf bytes win on disk.
    let skill = h.project().join(".claude/skills/write-tests/SKILL.md");
    let body = std::fs::read_to_string(&skill).unwrap();
    assert_eq!(body, "leaf impl", "leaf should shadow base");

    // `which` reports leaf as provider and base as shadowed.
    let out = h
        .cmd()
        .args(["which", ".claude/skills/write-tests/SKILL.md", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "which skill");
    let ws = stdout(&out);
    assert!(
        ws.contains("leaf::"),
        "which: should mention leaf namespace"
    );
    assert!(ws.contains("Shadows:"), "which: should report a shadow");
    assert!(
        ws.contains("base::"),
        "which: should mention base as shadowed"
    );
}

// ---------------------------------------------------------------------------
// Test 3: `aenv fork <file>` replaces symlink with a copy and drops state
// ---------------------------------------------------------------------------

#[test]
fn forking_a_managed_file_replaces_symlink_with_copy_and_drops_state() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");

    let home = h.aenv_home();
    write_file(&home.join("envs/base"), "CLAUDE.md", b"# base\n");
    std::fs::write(
        home.join("envs/base/aenv.toml"),
        b"name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let out = h
        .cmd()
        .args(["use", "base", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use base");
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "activate");

    let claude = h.project().join("CLAUDE.md");
    // Single-namespace CLAUDE.md is installed as a symlink.
    assert!(
        std::fs::symlink_metadata(&claude)
            .unwrap()
            .file_type()
            .is_symlink(),
        "single-namespace CLAUDE.md should be a symlink before fork"
    );

    // Fork the file.
    let out = h
        .cmd()
        .args(["fork", "CLAUDE.md", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "fork CLAUDE.md");

    // Symlink replaced by regular file with same content.
    let meta = std::fs::symlink_metadata(&claude).unwrap();
    assert!(
        !meta.file_type().is_symlink(),
        "fork should replace symlink with a regular file"
    );
    let body = std::fs::read_to_string(&claude).unwrap();
    assert_eq!(
        body, "# base\n",
        "forked file should contain original bytes"
    );
}

// ---------------------------------------------------------------------------
// Test 4: extends cycle exits with code 15
// ---------------------------------------------------------------------------

#[test]
fn extends_cycle_is_rejected_with_exit_15() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "a"]).output().unwrap();
    assert_success(&out, "create a");
    let out = h.cmd().args(["create", "b"]).output().unwrap();
    assert_success(&out, "create b");

    let home = h.aenv_home();
    // a extends b, b extends a — cycle.
    std::fs::write(
        home.join("envs/a/aenv.toml"),
        b"name = \"a\"\nextends = [\"b\"]\n",
    )
    .unwrap();
    std::fs::write(
        home.join("envs/b/aenv.toml"),
        b"name = \"b\"\nextends = [\"a\"]\n",
    )
    .unwrap();

    // Pin project to `a`.
    let out = h
        .cmd()
        .args(["use", "a", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use a");

    // Activate must fail with exit code 15 (ExtendsCycle).
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert!(!out.status.success(), "activation with a cycle should fail");
    assert_eq!(
        out.status.code(),
        Some(15),
        "cycle exit code must be 15; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("cycle") || stderr.contains("Cycle"),
        "stderr should mention 'cycle'; got: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 5: no materialized path contains "::" after activation
// ---------------------------------------------------------------------------

#[test]
fn no_materialized_path_contains_double_colon() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");
    let out = h.cmd().args(["create", "leaf"]).output().unwrap();
    assert_success(&out, "create leaf");

    let home = h.aenv_home();

    write_file(&home.join("envs/base"), "CLAUDE.md", b"# base\n");
    write_file(&home.join("envs/base"), ".claude/skills/x/SKILL.md", b"x");
    write_file(&home.join("envs/leaf"), "CLAUDE.md", b"# leaf\n");
    write_file(&home.join("envs/leaf"), ".claude/skills/y/SKILL.md", b"y");

    std::fs::write(
        home.join("envs/base/aenv.toml"),
        b"name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\", \".claude/skills/x/SKILL.md\"]\n",
    )
    .unwrap();
    std::fs::write(
        home.join("envs/leaf/aenv.toml"),
        b"name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\", \".claude/skills/y/SKILL.md\"]\n",
    )
    .unwrap();

    let out = h
        .cmd()
        .args(["use", "leaf", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use leaf");
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "activate");

    walk_for_double_colon(h.project());
}

fn walk_for_double_colon(root: &Path) {
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.unwrap();
        // Only check the path component relative to root to avoid false
        // positives from the tempdir prefix itself (which has no `::`).
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        let s = rel.to_string_lossy();
        assert!(
            !s.contains("::"),
            "materialized path '{}' contains '::' — identity-erasure invariant violated",
            entry.path().display()
        );
    }
}
