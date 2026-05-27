//! Activator runs `[lifecycle].on_activate` and rolls back materialization
//! on failure. Mirrors `activate_global_unit.rs`'s tempdir-as-fake-$HOME
//! pattern so nothing touches the developer's real environment.

use std::path::Path;

fn make_script(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }
}

#[test]
fn on_activate_success_records_lifecycle_ran_true() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

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
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"x").unwrap();
    let script = ns_dir.join("ok.sh");
    make_script(
        &script,
        "#!/bin/sh\ntouch \"$AENV_TARGET_ROOT/.aenv-ok-ran\"\nexit 0\n",
    );
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "ok.sh"
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let state = aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap();
    assert!(
        state.lifecycle_ran,
        "lifecycle_ran should be true after success"
    );
    assert!(
        fake_home.join(".aenv-ok-ran").exists(),
        "on_activate script should have run with AENV_TARGET_ROOT set"
    );
}

#[test]
fn on_activate_failure_rolls_back_materialization() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original").unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

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
    let script = ns_dir.join("fail.sh");
    make_script(&script, "#!/bin/sh\nexit 1\n");
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "fail.sh"
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let err = aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap_err();
    assert!(
        matches!(err, aenv_core::AenvError::GlobalConflict(_)),
        "expected GlobalConflict on on_activate failure, got {err:?}"
    );
    assert_eq!(err.exit_code(), 19);

    // Original file restored.
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"original"
    );
    // No state file.
    assert!(!aenv_home.join("global-state.json").exists());
}

#[test]
fn on_activate_missing_script_returns_manifest_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

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
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"x").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "missing.sh"
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let err = aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap_err();
    assert!(
        matches!(err, aenv_core::AenvError::ManifestInvalid(_)),
        "expected ManifestInvalid for missing script, got {err:?}"
    );
    assert!(!aenv_home.join("global-state.json").exists());
}

#[test]
fn no_lifecycle_means_lifecycle_ran_false() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

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
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"x").unwrap();
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
    let state = aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap();
    assert!(!state.lifecycle_ran);
}
