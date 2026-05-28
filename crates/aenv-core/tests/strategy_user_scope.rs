//! Strategy decisions for user-scope candidates.
//!
//! `decide_strategy` historically consulted `adapter.roles` and
//! `adapter.default_merge` with the bare candidate path as the lookup key.
//! User-scope candidates carry the same bare path (e.g. `.claude/CLAUDE.md`)
//! but the adapter's user-side maps are keyed by the `~/`-prefixed form
//! (`~/.claude/CLAUDE.md`). Without the scope branch in `decide_strategy`,
//! a multi-candidate user-scope CLAUDE.md would fall through to Symlink
//! instead of SectionMerge — silently breaking the extends-chain merge
//! for user content.

use std::path::PathBuf;

use aenv_core::adapter::Adapter;
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::{Candidate, MaterializeStrategy};
use aenv_core::scope::Scope;
use aenv_core::strategy::decide_strategy;

fn cc_with_user() -> Adapter {
    toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
user_files = ["~/.claude/CLAUDE.md", "~/.claude/settings.json"]

[roles]
"CLAUDE.md" = "instructions"

[user_roles]
"~/.claude/CLAUDE.md" = "instructions"

[user_default_merge]
"~/.claude/settings.json" = "deep"
"#,
    )
    .unwrap()
}

fn registry_with(adapter: Adapter) -> aenv_core::adapter::AdapterRegistry {
    let mut reg = aenv_core::adapter::AdapterRegistry::new();
    reg.insert(adapter);
    reg
}

fn ns(name: &str) -> NamespaceId {
    NamespaceId::new(name).unwrap()
}

fn user_candidate(ns_name: &str, path: &str) -> Candidate {
    Candidate {
        namespace: ns(ns_name),
        path: PathBuf::from(path),
        source_path: PathBuf::from(format!("/aenv/envs/{ns_name}/user/{path}")),
        adapter: "claude-code".into(),
        merge_override: None,
        skill_provenance: None,
        scope: Scope::User,
        adapter_materialize_override: None,
    }
}

#[test]
fn user_scope_instructions_role_yields_section_merge() {
    let adapters = registry_with(cc_with_user());
    let candidates = vec![
        user_candidate("base", ".claude/CLAUDE.md"),
        user_candidate("leaf", ".claude/CLAUDE.md"),
    ];
    let strategy = decide_strategy(&candidates, &adapters).unwrap();
    assert_eq!(strategy, MaterializeStrategy::SectionMerge);
}

#[test]
fn user_scope_default_merge_deep_yields_deep_merge_json() {
    let adapters = registry_with(cc_with_user());
    let candidates = vec![
        user_candidate("base", ".claude/settings.json"),
        user_candidate("leaf", ".claude/settings.json"),
    ];
    let strategy = decide_strategy(&candidates, &adapters).unwrap();
    assert!(
        matches!(strategy, MaterializeStrategy::DeepMerge(_)),
        "expected DeepMerge for user_default_merge=deep on .json, got {strategy:?}"
    );
}

#[test]
fn user_scope_with_no_matching_role_or_default_falls_back_to_symlink() {
    // A path the adapter declares no role and no default_merge for.
    let adapters = registry_with(cc_with_user());
    let candidates = vec![
        user_candidate("base", ".claude/agents/whatever.md"),
        user_candidate("leaf", ".claude/agents/whatever.md"),
    ];
    let strategy = decide_strategy(&candidates, &adapters).unwrap();
    assert_eq!(strategy, MaterializeStrategy::Symlink);
}

#[test]
fn project_scope_strategy_unchanged_by_user_branch() {
    // Regression guard: the scope branch must not affect project-scope behavior.
    let adapters = registry_with(cc_with_user());
    let candidates = vec![
        Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/aenv/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
            skill_provenance: None,
            scope: Scope::Project,
            adapter_materialize_override: None,
        },
        Candidate {
            namespace: ns("leaf"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/aenv/envs/leaf/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
            skill_provenance: None,
            scope: Scope::Project,
            adapter_materialize_override: None,
        },
    ];
    let strategy = decide_strategy(&candidates, &adapters).unwrap();
    assert_eq!(strategy, MaterializeStrategy::SectionMerge);
}
