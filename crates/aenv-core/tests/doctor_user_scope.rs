//! Doctor evaluator awareness of user-scope candidates.
//!
//! The `instructions_max_chars` policy (and its R-24 auto-fire helper) must
//! consult `adapter.user_roles` + `adapter.user_soft_limits` for any candidate
//! with `Scope::User`, and label outcomes with a `~/`-prefixed path so users
//! see what they'll see once activation lands.

#[test]
fn doctor_reports_user_scope_soft_limit_violation() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    // Tiny user-scope soft limit; project side is unconstrained.
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]

[user_roles]
"~/.claude/CLAUDE.md" = "instructions"

[user_soft_limits]
instructions = 10
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("oversize");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), "x".repeat(500)).unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "oversize"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("oversize").unwrap();
    let resolution =
        aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution);

    let labels: Vec<String> = report
        .outcomes
        .iter()
        .map(|o| {
            let t = o
                .target
                .as_ref()
                .map_or(String::new(), std::string::ToString::to_string);
            format!("{} {}", o.key, t)
        })
        .collect();
    assert!(
        labels
            .iter()
            .any(|l| l.contains("instructions_max_chars") && l.contains("~/.claude/CLAUDE.md")),
        "no user-scope soft-limit violation reported: {labels:?}"
    );
}
