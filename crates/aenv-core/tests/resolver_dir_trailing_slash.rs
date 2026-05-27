//! Trailing-slash regression: `user_files = [".claude/agents/"]` must
//! produce a candidate whose path has NO trailing slash, otherwise the
//! eventual `symlink()` call hits Linux's "resolve link path as directory"
//! semantics and fails with ENOENT.

use aenv_core::scope::Scope;

#[test]
fn user_files_trailing_slash_normalized_in_candidate_path() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/agents/"]
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude/agents")).unwrap();
    std::fs::write(
        ns_dir.join("user/.claude/agents/code-reviewer.md"),
        b"agent",
    )
    .unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/agents/"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let result = aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();

    let agents = result
        .candidates
        .iter()
        .find(|c| c.scope == Scope::User)
        .expect("user-scope candidate present");
    let path_str = agents.path.to_string_lossy();
    assert!(
        !path_str.ends_with('/'),
        "candidate path retained trailing slash: {path_str:?} \
         (would break symlink() at materialization time)"
    );
    assert_eq!(path_str, ".claude/agents");
}

#[test]
fn project_files_trailing_slash_normalized_in_candidate_path() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
files = [".claude/"]
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join(".claude/agents")).unwrap();
    std::fs::write(ns_dir.join(".claude/agents/explorer.md"), b"a").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
files = [".claude/"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let result = aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();

    let proj = result
        .candidates
        .iter()
        .find(|c| c.scope == Scope::Project)
        .expect("project-scope candidate present");
    let path_str = proj.path.to_string_lossy();
    assert!(
        !path_str.ends_with('/'),
        "project candidate path retained trailing slash: {path_str:?}"
    );
    assert_eq!(path_str, ".claude");
}
