//! Unit tests for `activate_namespace_in_scope(scope = User)`. Use a tempdir
//! as a fake $HOME so we never touch the developer's real ~/.claude.

#[test]
fn activate_user_scope_writes_files_under_home_and_state_under_aenv_home() {
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
user_files = ["~/.claude/CLAUDE.md", "~/.claude/agents/"]
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("research");
    std::fs::create_dir_all(ns_dir.join("user/.claude/agents")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"# Research mode").unwrap();
    std::fs::write(
        ns_dir.join("user/.claude/agents/explorer.md"),
        b"explorer body",
    )
    .unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "research"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md", ".claude/agents/explorer.md"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("research").unwrap();
    let state = aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap();

    assert_eq!(state.scope, aenv_core::scope::Scope::User);
    assert_eq!(state.active_namespace, "research");

    let claude_md = fake_home.join(".claude/CLAUDE.md");
    let agent = fake_home.join(".claude/agents/explorer.md");
    assert!(
        claude_md.exists(),
        "CLAUDE.md not materialized under $HOME: {claude_md:?}"
    );
    assert!(
        agent.exists(),
        "agent not materialized under $HOME: {agent:?}"
    );

    let state_path = aenv_home.join("global-state.json");
    assert!(
        state_path.exists(),
        "global-state.json not at AENV_HOME: {state_path:?}"
    );
    let body = std::fs::read_to_string(&state_path).unwrap();
    assert!(
        body.contains("\"scope\": \"user\""),
        "state file missing scope field: {body}"
    );
}

#[test]
fn user_scope_stashes_displaced_files_under_aenv_home() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    // Preexisting ~/.claude/CLAUDE.md that aenv must stash.
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(
        fake_home.join(".claude/CLAUDE.md"),
        b"original user CLAUDE.md",
    )
    .unwrap();

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
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new user CLAUDE.md").unwrap();
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

    assert_eq!(state.backed_up.len(), 1, "expected one stashed file");
    let stash = &state.backed_up[0].backup_path;
    assert!(
        stash.starts_with(&aenv_home),
        "stash path {stash:?} must live under aenv_home {aenv_home:?}, not under fake_home"
    );
    assert!(
        stash
            .components()
            .any(|c| c.as_os_str() == std::ffi::OsStr::new("global-stash")),
        "stash path {stash:?} must be under <aenv_home>/global-stash/"
    );
    let body = std::fs::read(stash).unwrap();
    assert_eq!(
        body, b"original user CLAUDE.md",
        "stash must contain the original bytes verbatim"
    );

    // And the new content landed at the target.
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"new user CLAUDE.md"
    );
}

#[test]
fn user_scope_swap_transactional() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

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

    for (name, body) in [("foo", b"foo body" as &[u8]), ("bar", b"bar body")] {
        let ns_dir = registry.namespace_dir(name);
        std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
        std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), body).unwrap();
        std::fs::write(
            ns_dir.join("aenv.toml"),
            format!(
                r#"
name = "{name}"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#
            ),
        )
        .unwrap();
    }

    let foo = aenv_core::identity::NamespaceId::new("foo").unwrap();
    let bar = aenv_core::identity::NamespaceId::new("bar").unwrap();

    aenv_core::activate::swap_or_activate_user(&fs, &registry, &adapters, &fake_home, &foo)
        .unwrap();
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"foo body"
    );

    aenv_core::activate::swap_or_activate_user(&fs, &registry, &adapters, &fake_home, &bar)
        .unwrap();
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"bar body"
    );

    aenv_core::deactivate::deactivate_namespace_in_scope(
        &fs,
        &registry,
        &fake_home,
        aenv_core::scope::Scope::User,
    )
    .unwrap();
    // The deepest restored layer is the original, not foo's body — only one
    // level of stash matters because the foo→bar deactivate restored the
    // pre-foo original, then bar's activate stashed THAT, and bar's deactivate
    // restored it. End state: original.
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"original"
    );
}

#[test]
fn user_scope_swap_rolls_back_when_new_namespace_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
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

    // Working namespace foo.
    {
        let ns_dir = registry.namespace_dir("foo");
        std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
        std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"foo body").unwrap();
        std::fs::write(
            ns_dir.join("aenv.toml"),
            r#"
name = "foo"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
        )
        .unwrap();
    }
    // Broken namespace bar — references a missing adapter.
    {
        let ns_dir = registry.namespace_dir("bar");
        std::fs::create_dir_all(&ns_dir).unwrap();
        std::fs::write(
            ns_dir.join("aenv.toml"),
            r#"
name = "bar"
[adapters.does-not-exist]
user_files = [".claude/foo.md"]
"#,
        )
        .unwrap();
    }

    let foo = aenv_core::identity::NamespaceId::new("foo").unwrap();
    let bar = aenv_core::identity::NamespaceId::new("bar").unwrap();
    aenv_core::activate::swap_or_activate_user(&fs, &registry, &adapters, &fake_home, &foo)
        .unwrap();
    let _err =
        aenv_core::activate::swap_or_activate_user(&fs, &registry, &adapters, &fake_home, &bar)
            .unwrap_err();
    let body = std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap();
    assert_eq!(
        body, b"foo body",
        "foo must be reactivated after bar failed"
    );
}

#[test]
fn concurrent_global_activation_rejects_with_exit_19() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
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

    // Simulate a held lock by acquiring one ourselves and never releasing.
    let lock_path = registry.global_lock_path();
    let _h = aenv_core::global_lock::acquire_global_lock(&lock_path).unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let err =
        aenv_core::activate::swap_or_activate_user(&fs, &registry, &adapters, &fake_home, &leaf)
            .unwrap_err();
    assert_eq!(err.exit_code(), 19);
    assert!(matches!(err, aenv_core::AenvError::GlobalConflict(_)));
}
