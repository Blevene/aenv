//! End-to-end tests for `aenv skill import-all` (issue #1): bulk-import every
//! `<base>/<subdir>/SKILL.md` from a monorepo as one `[[skills]]` entry each.
//! Driven as a subprocess against a local-fixture monorepo (offline).

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

/// Build a local monorepo with `skills/{alpha,beta,broken}/SKILL.md` — alpha and
/// beta are valid, broken has no `name:` frontmatter.
fn fixture(root: &Path) {
    for (name, body) in [
        ("alpha", "---\nname: alpha\ndescription: A\n---\n# alpha\n"),
        ("beta", "---\nname: beta\ndescription: B\n---\n# beta\n"),
        ("broken", "# broken, no frontmatter\n"),
    ] {
        let dir = root.join("skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), body).unwrap();
    }
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
fn imports_valid_skills_warns_on_malformed_and_materializes() {
    let home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    fixture(src.path());
    create_ns(&aenv_home, "mono");

    let out = aenv(&aenv_home)
        .args(["skill", "import-all"])
        .arg(src.path())
        .args(["--ns", "mono"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "import-all failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("broken") && stderr.contains("frontmatter"),
        "expected a warning about the malformed skill; got: {stderr}"
    );

    let m = manifest(&aenv_home, "mono");
    assert!(m.contains("name = \"alpha\"") && m.contains("name = \"beta\""));
    assert!(
        !m.contains("name = \"broken\""),
        "malformed skill must not be declared"
    );
    assert!(m.contains("path = \"skills/alpha\""));

    // The bulk-imported skills materialize like any imported skill.
    let proj = tempdir().unwrap();
    assert!(aenv(&aenv_home)
        .args(["use", "mono", "--project"])
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
    assert!(proj.path().join(".claude/skills/alpha/SKILL.md").exists());
    assert!(proj.path().join(".claude/skills/beta/SKILL.md").exists());
}

#[test]
fn only_filters_to_named_subset() {
    let home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    fixture(src.path());
    create_ns(&aenv_home, "mono");

    assert!(aenv(&aenv_home)
        .args(["skill", "import-all"])
        .arg(src.path())
        .args(["--ns", "mono", "--only", "alpha"])
        .status()
        .unwrap()
        .success());
    let m = manifest(&aenv_home, "mono");
    assert!(m.contains("name = \"alpha\""));
    assert!(
        !m.contains("name = \"beta\""),
        "beta should be filtered out"
    );
}

#[test]
fn only_unknown_name_errors_before_write() {
    let home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    fixture(src.path());
    create_ns(&aenv_home, "mono");

    let out = aenv(&aenv_home)
        .args(["skill", "import-all"])
        .arg(src.path())
        .args(["--ns", "mono", "--only", "nope"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not found"));
    // No skills written.
    assert!(!manifest(&aenv_home, "mono").contains("[[skills]]"));
}

#[test]
fn idempotent_rerun_skips_already_declared() {
    let home = tempdir().unwrap();
    let src = tempdir().unwrap();
    let aenv_home = home.path().join(".aenv");
    fixture(src.path());
    create_ns(&aenv_home, "mono");

    let first = aenv(&aenv_home)
        .args(["skill", "import-all"])
        .arg(src.path())
        .args(["--ns", "mono"])
        .output()
        .unwrap();
    assert!(first.status.success());
    let after_first = manifest(&aenv_home, "mono");

    let second = aenv(&aenv_home)
        .args(["skill", "import-all"])
        .arg(src.path())
        .args(["--ns", "mono"])
        .output()
        .unwrap();
    assert!(second.status.success());
    assert!(String::from_utf8_lossy(&second.stdout).contains("already declared"));
    // Manifest unchanged on the second run.
    assert_eq!(after_first, manifest(&aenv_home, "mono"));
}
