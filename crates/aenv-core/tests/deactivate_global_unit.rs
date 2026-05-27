//! Unit tests for `deactivate_namespace_in_scope(scope = User)`. Uses a tempdir
//! as a fake $HOME.

#[test]
fn deactivate_user_scope_restores_stash_and_removes_state() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    // Pre-existing ~/.claude/CLAUDE.md the activate run will stash.
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original").unwrap();

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap();

    // Sanity: new content is in place.
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"new"
    );

    let active = aenv_core::deactivate::deactivate_namespace_in_scope(
        &fs,
        &registry,
        &fake_home,
        aenv_core::scope::Scope::User,
    )
    .unwrap();
    assert_eq!(active, "ns");

    // Original bytes restored.
    let restored = std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap();
    assert_eq!(restored, b"original");

    // State file is gone.
    assert!(!aenv_home.join("global-state.json").exists());

    // The aenv_home itself (registry root) still exists with its adapter and env directories.
    assert!(adapters_dir.exists());
    assert!(ns_dir.exists());
}

#[test]
fn deactivate_user_scope_with_no_state_returns_activation_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;
    let err = aenv_core::deactivate::deactivate_namespace_in_scope(
        &fs,
        &registry,
        &fake_home,
        aenv_core::scope::Scope::User,
    )
    .unwrap_err();
    assert!(
        matches!(err, aenv_core::AenvError::ActivationConflict(_)),
        "expected ActivationConflict, got {err:?}"
    );
}
