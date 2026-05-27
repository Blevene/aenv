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
