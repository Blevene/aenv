//! End-to-end tests for the opt-in `--shared` flag (issue #5 follow-up):
//! `create --global`, `global new`, `global snapshot`, and `global import`
//! can emit `shared_files` (one stored copy serving both scopes) instead of
//! `user_files` (global only). Driven as a subprocess with `AENV_HOME` and
//! `HOME` pointed at tempdirs so the real `$HOME` is never touched.

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

/// `aenv create <ns> --global --shared` writes `shared_files` (not `user_files`),
/// and the one stored copy then materializes into BOTH scopes.
#[test]
fn create_global_shared_emits_shared_files_and_serves_both_scopes() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");

    let out = aenv(&aenv_home, fake_home.path())
        .args(["create", "dual", "--global", "--shared"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "create --global --shared failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let manifest = std::fs::read_to_string(aenv_home.join("envs/dual/aenv.toml")).unwrap();
    assert!(
        manifest.contains("shared_files"),
        "manifest should declare shared_files; got:\n{manifest}"
    );
    assert!(
        !manifest.contains("user_files"),
        "manifest should NOT declare user_files with --shared; got:\n{manifest}"
    );

    // Put a distinctive marker in the single source.
    let src = aenv_home.join("envs/dual/user/.claude/CLAUDE.md");
    std::fs::write(&src, "# dual\nDUAL-MARKER\n").unwrap();

    // Global scope.
    assert!(aenv(&aenv_home, fake_home.path())
        .args(["activate", "dual", "--global", "--yes"])
        .status()
        .unwrap()
        .success());
    assert!(
        std::fs::read_to_string(fake_home.path().join(".claude/CLAUDE.md"))
            .unwrap()
            .contains("DUAL-MARKER")
    );

    // Project scope — instructions remap to repo-root CLAUDE.md.
    aenv(&aenv_home, fake_home.path())
        .args(["use", "dual"])
        .arg("--project")
        .arg(project.path())
        .status()
        .unwrap();
    assert!(aenv(&aenv_home, fake_home.path())
        .args(["activate"])
        .arg("--project")
        .arg(project.path())
        .status()
        .unwrap()
        .success());
    assert!(std::fs::read_to_string(project.path().join("CLAUDE.md"))
        .unwrap()
        .contains("DUAL-MARKER"));
}

/// `--shared` without `--global` is rejected up front (it controls user-scope
/// scaffolding only).
#[test]
fn shared_without_global_is_rejected() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");

    let out = aenv(&aenv_home, fake_home.path())
        .args(["create", "p", "--shared"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "--shared without --global should fail"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--shared applies only with --global"),
        "expected guard message; got: {stderr}"
    );
}

/// `aenv global snapshot <ns> --shared` captures the current `$HOME` surface
/// into `shared_files`.
#[test]
fn global_snapshot_shared_emits_shared_files() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    // Bootstrap + author a $HOME surface to capture.
    std::fs::create_dir_all(fake_home.path().join(".claude")).unwrap();
    std::fs::write(fake_home.path().join(".claude/CLAUDE.md"), "# mine\n").unwrap();

    let out = aenv(&aenv_home, fake_home.path())
        .args(["global", "snapshot", "snap", "--shared"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global snapshot --shared failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let manifest = std::fs::read_to_string(aenv_home.join("envs/snap/aenv.toml")).unwrap();
    assert!(
        manifest.contains("shared_files") && !manifest.contains("user_files"),
        "snapshot --shared should declare shared_files only; got:\n{manifest}"
    );
}

/// `aenv global import <local-dir> <ns> --shared` captures an external tree into
/// `shared_files`.
#[test]
fn global_import_shared_emits_shared_files() {
    let home = tempdir().unwrap();
    let fake_home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    // A minimal importable source tree the heuristic recognizes: `CLAUDE.md`
    // at the source root maps to `.claude/CLAUDE.md` under user/.
    std::fs::write(src.path().join("CLAUDE.md"), "# imported\n").unwrap();

    let out = aenv(&aenv_home, fake_home.path())
        .args(["global", "import"])
        .arg(src.path())
        .args(["imp", "--shared"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global import --shared failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let manifest = std::fs::read_to_string(aenv_home.join("envs/imp/aenv.toml")).unwrap();
    assert!(
        manifest.contains("shared_files") && !manifest.contains("user_files"),
        "import --shared should declare shared_files only; got:\n{manifest}"
    );
}
