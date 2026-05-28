//! End-to-end rescue tests. We invoke the aenv-rescue binary directly via
//! its CARGO_BIN_EXE env var; the test fixture constructs an active global
//! state by hand (no need to involve the main aenv binary).

use std::path::Path;
use std::process::Command;

fn rescue() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv-rescue"))
}

fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn rescue_with_no_active_state_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();

    let out = rescue()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("No active global activation"),
        "expected no-activation marker, got: {stdout}"
    );
}

#[test]
fn rescue_restores_after_simulated_lockout() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();

    // Construct an active state by hand. Original CLAUDE.md → stash.
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"new active content").unwrap();

    let stash = aenv_home.join("global-stash/epoch-test/.claude");
    std::fs::create_dir_all(&stash).unwrap();
    std::fs::write(stash.join("CLAUDE.md"), b"original user content").unwrap();

    let state = aenv_core::state::ActivationState {
        schema_version: aenv_core::state::SCHEMA_VERSION,
        scope: aenv_core::scope::Scope::User,
        active_namespace: "test-ns".into(),
        project_root: fake_home.clone(),
        managed_files: vec![aenv_core::state::ManagedFile {
            path: ".claude/CLAUDE.md".into(),
            qualified_name: aenv_core::identity::QualifiedName::new(
                aenv_core::identity::NamespaceId::new("test-ns").unwrap(),
                aenv_core::identity::ShortName::new(".claude/CLAUDE.md".to_string()).unwrap(),
            ),
            strategy: aenv_core::resolve::MaterializeStrategy::Symlink,
            contributors: vec![],
            shadows: vec![],
            skill_provenance: None,
            was_present_before_activation: true,
        }],
        backed_up: vec![aenv_core::state::BackedUpFile {
            original_path: ".claude/CLAUDE.md".into(),
            backup_path: stash.join("CLAUDE.md"),
        }],
        parameters: Default::default(),
        policies: Default::default(),
        warnings: vec![],
        lifecycle_ran: false,
    };
    std::fs::write(
        aenv_home.join("global-state.json"),
        state.to_json().unwrap(),
    )
    .unwrap();

    let out = rescue()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "rescue failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Original restored.
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"original user content"
    );
    // State + lock removed.
    assert!(!aenv_home.join("global-state.json").exists());
    assert!(!aenv_home.join("global.lock").exists());
}

#[test]
fn rescue_handles_absent_was_present_false() {
    // Simulate a state where a managed file was added to an empty slot
    // (was_present_before_activation = false). Rescue should remove the
    // materialized file but NOT try to restore anything.
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"materialized by aenv").unwrap();

    let state = aenv_core::state::ActivationState {
        schema_version: aenv_core::state::SCHEMA_VERSION,
        scope: aenv_core::scope::Scope::User,
        active_namespace: "test-ns".into(),
        project_root: fake_home.clone(),
        managed_files: vec![aenv_core::state::ManagedFile {
            path: ".claude/CLAUDE.md".into(),
            qualified_name: aenv_core::identity::QualifiedName::new(
                aenv_core::identity::NamespaceId::new("test-ns").unwrap(),
                aenv_core::identity::ShortName::new(".claude/CLAUDE.md".to_string()).unwrap(),
            ),
            strategy: aenv_core::resolve::MaterializeStrategy::Symlink,
            contributors: vec![],
            shadows: vec![],
            skill_provenance: None,
            was_present_before_activation: false, // Absent-case
        }],
        backed_up: vec![],
        parameters: Default::default(),
        policies: Default::default(),
        warnings: vec![],
        lifecycle_ran: false,
    };
    std::fs::write(
        aenv_home.join("global-state.json"),
        state.to_json().unwrap(),
    )
    .unwrap();

    let out = rescue()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .output()
        .unwrap();
    assert!(out.status.success());

    // Materialized file is removed; nothing replaces it (was Absent).
    assert!(!fake_home.join(".claude/CLAUDE.md").exists());
    assert!(!aenv_home.join("global-state.json").exists());
}
