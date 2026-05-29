//! End-to-end tests for `aenv global new <name>` — scaffolding an editable
//! user-scope namespace from scratch.

use std::path::Path;
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn global_new_scaffolds_editable_namespace_then_use_activates() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();

    // Scaffold from a completely fresh registry (no prior `aenv create`).
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "new", "mine"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global new failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The instructions file is seeded under user/ and declared in the manifest.
    let seeded = aenv_home.join("envs/mine/user/.claude/CLAUDE.md");
    assert!(seeded.exists(), "seeded CLAUDE.md missing at {seeded:?}");
    let manifest = std::fs::read_to_string(aenv_home.join("envs/mine/aenv.toml")).unwrap();
    assert!(
        manifest.contains(".claude/CLAUDE.md"),
        "manifest should declare the seeded user file: {manifest}"
    );

    // The scaffold is immediately usable: activating it materializes the file.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "activate", "mine", "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global activate after new failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        fake_home.join(".claude/CLAUDE.md").exists(),
        "namespace did not materialize under $HOME after activate"
    );
}

#[test]
fn global_new_refuses_existing_namespace() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();

    let mk = || {
        aenv()
            .env("AENV_HOME", &aenv_home)
            .env("HOME", &fake_home)
            .args(["global", "new", "dup"])
            .output()
            .unwrap()
    };
    assert!(mk().status.success(), "first new should succeed");
    let second = mk();
    assert!(
        !second.status.success(),
        "second new with same name should fail"
    );
}
