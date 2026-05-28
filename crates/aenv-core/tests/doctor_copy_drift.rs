//! Doctor reports `copy_mode_local_edits` outcomes when a Copy-strategy
//! managed file has been edited on disk since the activation that materialized it.

#[test]
fn doctor_warns_on_local_edit_to_copy_target() {
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
materialize = "copy"
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"namespace content").unwrap();
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

    // Edit the materialized file locally.
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"my local edits").unwrap();

    let resolution =
        aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution, &fake_home);

    let drift_warnings: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| o.key == "copy_mode_local_edits")
        .collect();
    assert_eq!(
        drift_warnings.len(),
        1,
        "expected 1 drift warning, got {:?}",
        report.outcomes
    );
    assert!(matches!(
        drift_warnings[0].status,
        aenv_core::policies::builtin::OutcomeStatus::Warn { .. }
    ));
    // The synthesized target must pass the global-doctor `::~/` filter.
    let target_str = drift_warnings[0]
        .target
        .as_ref()
        .map(std::string::ToString::to_string)
        .unwrap_or_default();
    assert!(
        target_str.contains("::~/"),
        "target should pass `::~/` filter: {target_str}"
    );
}

#[test]
fn doctor_silent_when_copy_target_unchanged() {
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
materialize = "copy"
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"namespace content").unwrap();
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

    let resolution =
        aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution, &fake_home);

    let drift_warnings: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| o.key == "copy_mode_local_edits")
        .collect();
    assert!(
        drift_warnings.is_empty(),
        "expected no drift warnings, got {drift_warnings:?}"
    );
}

#[test]
fn doctor_silent_for_symlink_strategy_namespace() {
    // Symlink-strategy namespace (no materialize="copy"). Edits flow back to
    // the namespace source; this isn't the drift class we warn about.
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
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"namespace content").unwrap();
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

    let resolution =
        aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution, &fake_home);

    let drift_warnings: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| o.key == "copy_mode_local_edits")
        .collect();
    assert!(
        drift_warnings.is_empty(),
        "symlink strategy should not produce copy_mode_local_edits warnings"
    );
}

#[test]
fn doctor_silent_when_no_active_state() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
materialize = "copy"
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"content").unwrap();
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
    let resolution =
        aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution, &fake_home);

    let drift_warnings: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| o.key == "copy_mode_local_edits")
        .collect();
    assert!(
        drift_warnings.is_empty(),
        "no active state should mean no drift warnings: {drift_warnings:?}"
    );
}
