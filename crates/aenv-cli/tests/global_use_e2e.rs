//! End-to-end tests for `aenv global use <target>` — the one-command front
//! door that folds import + activate + swap (and a `-` toggle) into one verb.

use std::path::Path;
use std::process::Command;

fn aenv(aenv_home: &Path, fake_home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
    c.env("AENV_HOME", aenv_home).env("HOME", fake_home);
    c
}

fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

/// Minimal claude-code adapter so the registry resolves user_files.
fn seed_adapter(aenv_home: &Path) {
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        "name = \"claude-code\"\nuser_files = [\"~/.claude/CLAUDE.md\"]\n",
    )
    .unwrap();
}

/// Hand-author a namespace `name` whose user payload is `body`.
fn seed_namespace(aenv_home: &Path, name: &str, body: &str) {
    let ns = aenv_home.join("envs").join(name);
    std::fs::create_dir_all(ns.join("user/.claude")).unwrap();
    std::fs::write(ns.join("user/.claude/CLAUDE.md"), body).unwrap();
    std::fs::write(
        ns.join("aenv.toml"),
        format!(
            "name = \"{name}\"\n[adapters.claude-code]\nuser_files = [\".claude/CLAUDE.md\"]\n"
        ),
    )
    .unwrap();
}

/// A claude-ctrl-style local source tree the importer can pick up heuristically.
fn seed_source_tree(dir: &Path, body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("CLAUDE.md"), body).unwrap();
}

#[test]
fn use_local_source_imports_and_activates_in_one_command() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_adapter(&aenv_home);
    let src = canon(tmp.path()).join("ctrl-src");
    seed_source_tree(&src, "imported profile");

    let out = aenv(&aenv_home, &fake_home)
        .args([
            "global",
            "use",
            src.to_str().unwrap(),
            "--as",
            "ctrl",
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "use <source> failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Imported as a namespace AND activated under $HOME in one shot.
    assert!(
        aenv_home.join("envs/ctrl/aenv.toml").exists(),
        "not imported"
    );
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"imported profile",
        "source content not materialized under $HOME"
    );
}

#[test]
fn use_existing_name_switches() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_adapter(&aenv_home);
    seed_namespace(&aenv_home, "alpha", "alpha body");

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "use", "alpha", "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"alpha body"
    );
}

#[test]
fn use_dash_toggles_to_previous_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_adapter(&aenv_home);
    seed_namespace(&aenv_home, "alpha", "alpha body");
    seed_namespace(&aenv_home, "beta", "beta body");

    // Use alpha, then beta — previous becomes alpha.
    for ns in ["alpha", "beta"] {
        assert!(aenv(&aenv_home, &fake_home)
            .args(["global", "use", ns, "--yes", "--no-baseline"])
            .status()
            .unwrap()
            .success());
    }
    // `use -` should switch back to alpha.
    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "use", "-", "--yes", "--no-baseline"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "use - failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"alpha body",
        "use - did not restore the previous profile"
    );
}

#[test]
fn use_dash_with_no_previous_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_adapter(&aenv_home);

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "use", "-", "--yes"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "use - with no history should fail");
}

#[test]
fn activate_alias_still_works() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_adapter(&aenv_home);
    seed_namespace(&aenv_home, "alpha", "alpha body");

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "alpha", "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(fake_home.join(".claude/CLAUDE.md").exists());
}
