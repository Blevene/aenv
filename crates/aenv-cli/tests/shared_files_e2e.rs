//! End-to-end test for `shared_files` (issue #5, Layer 2): one stored copy of a
//! profile's content, under the namespace's `user/` tree, serving BOTH scopes.
//!
//! `aenv activate <ns> --global` materialises it under `$HOME`; `aenv activate`
//! in a pinned project materialises the SAME bytes to the project, with the
//! role-tagged instructions file remapped from `.claude/CLAUDE.md` (user layout)
//! to repo-root `CLAUDE.md` (project layout). Driven as a subprocess with
//! `AENV_HOME` and `HOME` pointed at tempdirs so the real `$HOME` is untouched.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

fn aenv(aenv_home: &Path, fake_home: &Path) -> Command {
    let mut c = Command::new(bin());
    c.env("AENV_HOME", aenv_home).env("HOME", fake_home);
    c
}

/// Hand-author a namespace whose single stored content (under `user/`) is
/// declared `shared_files`, so it serves both scopes from one copy.
fn author_shared_namespace(aenv_home: &Path) {
    let ns = aenv_home.join("envs/shareprof");
    std::fs::create_dir_all(ns.join("user/.claude/agents")).unwrap();
    std::fs::write(
        ns.join("aenv.toml"),
        r#"name = "shareprof"
[adapters.claude-code]
shared_files = [".claude/CLAUDE.md", ".claude/agents/helper.md"]
"#,
    )
    .unwrap();
    std::fs::write(
        ns.join("user/.claude/CLAUDE.md"),
        "# shareprof\nSHARED-MARKER\n",
    )
    .unwrap();
    std::fs::write(
        ns.join("user/.claude/agents/helper.md"),
        "HELPER-MARKER-v1\n",
    )
    .unwrap();
}

#[test]
fn shared_files_one_copy_serves_both_scopes() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    author_shared_namespace(&aenv_home);

    // Exactly one stored copy: no duplicate at the namespace root.
    assert!(
        !aenv_home.join("envs/shareprof/CLAUDE.md").exists()
            && !aenv_home.join("envs/shareprof/.claude").exists(),
        "content must live only under user/, not duplicated at the namespace root"
    );

    // --- global scope: materialises under $HOME in the user layout ---
    let act_g = aenv(&aenv_home, fake_home.path())
        .args(["activate", "shareprof", "--global", "--yes"])
        .output()
        .unwrap();
    assert!(
        act_g.status.success(),
        "activate --global failed: {}",
        String::from_utf8_lossy(&act_g.stderr)
    );
    let home_md = fake_home.path().join(".claude/CLAUDE.md");
    let home_helper = fake_home.path().join(".claude/agents/helper.md");
    assert!(
        std::fs::read_to_string(&home_md)
            .unwrap()
            .contains("SHARED-MARKER"),
        "~/.claude/CLAUDE.md should carry the shared instructions"
    );
    assert!(
        std::fs::read_to_string(&home_helper)
            .unwrap()
            .contains("HELPER-MARKER-v1"),
        "~/.claude/agents/helper.md should be materialised"
    );

    // --- project scope: SAME content, instructions remapped to repo root ---
    // `use` pins the namespace at the project root (passed via --project <path>).
    let use_p = aenv(&aenv_home, fake_home.path())
        .args(["use", "shareprof"])
        .arg("--project")
        .arg(project.path())
        .output()
        .unwrap();
    assert!(
        use_p.status.success(),
        "use --project failed: {}",
        String::from_utf8_lossy(&use_p.stderr)
    );
    let act_p = aenv(&aenv_home, fake_home.path())
        .args(["activate"])
        .arg("--project")
        .arg(project.path())
        .output()
        .unwrap();
    assert!(
        act_p.status.success(),
        "project activate failed: {}",
        String::from_utf8_lossy(&act_p.stderr)
    );

    // Instructions file lands at repo-root CLAUDE.md (NOT ./.claude/CLAUDE.md).
    let proj_md = project.path().join("CLAUDE.md");
    assert!(
        std::fs::read_to_string(&proj_md)
            .unwrap()
            .contains("SHARED-MARKER"),
        "project CLAUDE.md (repo root) should carry the shared instructions"
    );
    assert!(
        !project.path().join(".claude/CLAUDE.md").exists(),
        "instructions must remap to repo root, not ./.claude/CLAUDE.md"
    );
    // The symmetric, non-role file keeps its layout in both scopes.
    let proj_helper = project.path().join(".claude/agents/helper.md");
    assert!(
        std::fs::read_to_string(&proj_helper)
            .unwrap()
            .contains("HELPER-MARKER-v1"),
        "project .claude/agents/helper.md should be materialised"
    );

    // --- shared-edit: editing the single source reflects in both scopes ---
    // helper.md is symlinked (non-role), so a source edit is visible immediately
    // through both materialised paths — proving one stored copy, not two.
    std::fs::write(
        aenv_home.join("envs/shareprof/user/.claude/agents/helper.md"),
        "HELPER-MARKER-v2\n",
    )
    .unwrap();
    assert!(
        std::fs::read_to_string(&home_helper)
            .unwrap()
            .contains("v2"),
        "global symlink should reflect the single-source edit"
    );
    assert!(
        std::fs::read_to_string(&proj_helper)
            .unwrap()
            .contains("v2"),
        "project symlink should reflect the same single-source edit"
    );
}
