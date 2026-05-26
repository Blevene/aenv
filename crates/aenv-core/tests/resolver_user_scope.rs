//! Resolver behavior under `Scope::User`: gathering candidates from
//! `envs/<ns>/user/` and tagging them as `Scope::User`.

use aenv_core::scope::Scope;

#[test]
fn resolver_emits_user_scope_candidates() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
user_files = ["~/.claude/CLAUDE.md", "~/.claude/agents/"]
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("foo");
    std::fs::create_dir_all(ns_dir.join("user/.claude/agents")).unwrap();
    std::fs::write(ns_dir.join("CLAUDE.md"), b"project").unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"user").unwrap();
    std::fs::write(ns_dir.join("user/.claude/agents/reviewer.md"), b"agent").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "foo"
[adapters.claude-code]
files = ["CLAUDE.md"]
user_files = [".claude/CLAUDE.md", ".claude/agents/reviewer.md"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("foo").unwrap();
    let result = aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();

    let mut project_paths: Vec<String> = result
        .candidates
        .iter()
        .filter(|c| c.scope == Scope::Project)
        .map(|c| c.path.to_string_lossy().into_owned())
        .collect();
    let mut user_paths: Vec<String> = result
        .candidates
        .iter()
        .filter(|c| c.scope == Scope::User)
        .map(|c| c.path.to_string_lossy().into_owned())
        .collect();
    project_paths.sort();
    user_paths.sort();
    assert_eq!(project_paths, vec!["CLAUDE.md".to_string()]);
    assert_eq!(
        user_paths,
        vec![
            ".claude/CLAUDE.md".to_string(),
            ".claude/agents/reviewer.md".to_string(),
        ]
    );

    // Spot-check source_path: user candidates root under ns_dir/user/.
    let user_claude = result
        .candidates
        .iter()
        .find(|c| c.scope == Scope::User && c.path.ends_with(".claude/CLAUDE.md"))
        .unwrap();
    assert!(
        user_claude.source_path.starts_with(ns_dir.join("user")),
        "user candidate source_path {:?} must live under ns_dir/user/",
        user_claude.source_path
    );
}

#[test]
fn user_scope_path_with_tilde_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
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
    let ns_dir = registry.namespace_dir("bad");
    std::fs::create_dir_all(&ns_dir).unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "bad"
[adapters.claude-code]
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("bad").unwrap();
    let err = aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("~/"), "expected '~/' rejection, got {msg}");
}
