//! End-to-end tests for `aenv vendor` (issue #2): copy non-skill content from a
//! local source into a namespace, declare it under `files`, record `[[vendored]]`
//! provenance, and materialize it on activation. Driven as a subprocess.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

fn aenv(aenv_home: &Path) -> Command {
    let mut c = Command::new(bin());
    c.env("AENV_HOME", aenv_home);
    c
}

fn source(root: &Path) {
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::create_dir_all(root.join("references")).unwrap();
    std::fs::write(root.join("agents/a.md"), "agent A\n").unwrap();
    std::fs::write(root.join("agents/b.md"), "agent B\n").unwrap();
    std::fs::write(root.join("references/r.md"), "ref R\n").unwrap();
}

fn create_ns(aenv_home: &Path, name: &str) {
    assert!(aenv(aenv_home)
        .args(["create", name, "--adapter", "claude-code"])
        .status()
        .unwrap()
        .success());
}

fn manifest(aenv_home: &Path, ns: &str) -> String {
    std::fs::read_to_string(aenv_home.join(format!("envs/{ns}/aenv.toml"))).unwrap()
}

#[test]
fn vendor_directory_declares_files_records_provenance_and_materializes() {
    let home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    source(src.path());
    create_ns(&aenv_home, "addy");

    let out = aenv(&aenv_home)
        .args(["vendor"])
        .arg(src.path())
        .args(["--ns", "addy", "--path", "agents", "--as", ".claude/agents"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "vendor failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Both files copied into the namespace tree.
    assert!(aenv_home.join("envs/addy/.claude/agents/a.md").exists());
    assert!(aenv_home.join("envs/addy/.claude/agents/b.md").exists());
    // Declared under files + recorded in [[vendored]].
    let m = manifest(&aenv_home, "addy");
    assert!(m.contains(".claude/agents/a.md") && m.contains(".claude/agents/b.md"));
    assert!(m.contains("[[vendored]]") && m.contains("src_path = \"agents\""));

    // Activation symlinks them like any project-scope file.
    let proj = tempdir().unwrap();
    assert!(aenv(&aenv_home)
        .args(["use", "addy", "--project"])
        .arg(proj.path())
        .status()
        .unwrap()
        .success());
    assert!(aenv(&aenv_home)
        .args(["activate", "--project"])
        .arg(proj.path())
        .status()
        .unwrap()
        .success());
    assert!(proj.path().join(".claude/agents/a.md").exists());
    assert!(proj.path().join(".claude/agents/b.md").exists());
}

#[test]
fn vendor_single_file() {
    let home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    source(src.path());
    create_ns(&aenv_home, "addy");

    assert!(aenv(&aenv_home)
        .args(["vendor"])
        .arg(src.path())
        .args([
            "--ns",
            "addy",
            "--path",
            "references/r.md",
            "--as",
            ".claude/references/r.md",
        ])
        .status()
        .unwrap()
        .success());
    assert!(aenv_home.join("envs/addy/.claude/references/r.md").exists());
    assert!(manifest(&aenv_home, "addy").contains(".claude/references/r.md"));
}

#[test]
fn vendor_rerun_is_idempotent_and_reports_drift() {
    let home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    source(src.path());
    create_ns(&aenv_home, "addy");

    let args = ["vendor"];
    let common = ["--ns", "addy", "--path", "agents", "--as", ".claude/agents"];
    assert!(aenv(&aenv_home)
        .args(args)
        .arg(src.path())
        .args(common)
        .status()
        .unwrap()
        .success());
    let after_first = manifest(&aenv_home, "addy");

    // Re-run unchanged: manifest identical, output reports no drift.
    let rerun = aenv(&aenv_home)
        .args(args)
        .arg(src.path())
        .args(common)
        .output()
        .unwrap();
    assert!(rerun.status.success());
    assert_eq!(after_first, manifest(&aenv_home, "addy"));

    // Edit the source → re-vendor flags the changed file.
    std::fs::write(src.path().join("agents/a.md"), "agent A v2\n").unwrap();
    let drift = aenv(&aenv_home)
        .args(args)
        .arg(src.path())
        .args(common)
        .output()
        .unwrap();
    assert!(drift.status.success());
    let stdout = String::from_utf8_lossy(&drift.stdout);
    assert!(
        stdout.contains("+ .claude/agents/a.md"),
        "expected a.md flagged as changed; got: {stdout}"
    );
    assert_eq!(
        std::fs::read_to_string(aenv_home.join("envs/addy/.claude/agents/a.md")).unwrap(),
        "agent A v2\n"
    );
}

#[test]
fn vendor_collision_errors_without_force() {
    let home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    source(src.path());
    create_ns(&aenv_home, "addy");
    // A pre-existing, unrelated file at the target.
    std::fs::create_dir_all(aenv_home.join("envs/addy/.claude")).unwrap();
    std::fs::write(aenv_home.join("envs/addy/.claude/notes.md"), "mine\n").unwrap();

    let out = aenv(&aenv_home)
        .args(["vendor"])
        .arg(src.path())
        .args([
            "--ns",
            "addy",
            "--path",
            "references/r.md",
            "--as",
            ".claude/notes.md",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already exists"));
    // Untouched.
    assert_eq!(
        std::fs::read_to_string(aenv_home.join("envs/addy/.claude/notes.md")).unwrap(),
        "mine\n"
    );

    // --force overwrites.
    assert!(aenv(&aenv_home)
        .args(["vendor"])
        .arg(src.path())
        .args([
            "--ns",
            "addy",
            "--path",
            "references/r.md",
            "--as",
            ".claude/notes.md",
            "--force",
        ])
        .status()
        .unwrap()
        .success());
    assert_eq!(
        std::fs::read_to_string(aenv_home.join("envs/addy/.claude/notes.md")).unwrap(),
        "ref R\n"
    );
}
