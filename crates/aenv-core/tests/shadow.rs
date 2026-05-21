use std::path::PathBuf;

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::{Candidate, MaterializeStrategy};
use aenv_core::shadow::compute_shadows;

fn cand(ns: &str, path: &str, adapter: &str) -> Candidate {
    Candidate {
        namespace: NamespaceId::new(ns).unwrap(),
        path: PathBuf::from(path),
        source_path: PathBuf::from(format!("/aenv/envs/{ns}/{path}")),
        adapter: adapter.to_string(),
        merge_override: None,
    }
}

fn qn(ns: &str, short: &str) -> QualifiedName {
    let nsid = if ns == NamespaceId::RESERVED_MERGED {
        NamespaceId::merged_synthetic()
    } else {
        NamespaceId::new(ns).unwrap()
    };
    QualifiedName::new(nsid, ShortName::new(short).unwrap())
}

fn cc_with_instructions() -> AdapterRegistry {
    let cc: Adapter = toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md", ".claude/skills/**/*"]
[roles]
"CLAUDE.md" = "instructions"
"#,
    )
    .unwrap();
    let mut r = AdapterRegistry::default();
    r.insert(cc);
    r
}

#[test]
fn symlink_path_with_two_candidates_yields_one_shadow() {
    let candidates = vec![
        cand("base", ".claude/skills/write-tests/SKILL.md", "claude-code"),
        cand("leaf", ".claude/skills/write-tests/SKILL.md", "claude-code"),
    ];
    let strategy = MaterializeStrategy::Symlink;
    let shadows = compute_shadows(&candidates, strategy, &cc_with_instructions()).unwrap();
    assert_eq!(shadows, vec![qn("base", ".claude/skills/write-tests/SKILL.md")]);
}

#[test]
fn three_deep_chain_yields_two_shadows_in_root_to_near_order() {
    let candidates = vec![
        cand("a", "X", "claude-code"),
        cand("b", "X", "claude-code"),
        cand("c", "X", "claude-code"),
    ];
    let shadows =
        compute_shadows(&candidates, MaterializeStrategy::Symlink, &cc_with_instructions()).unwrap();
    assert_eq!(shadows.len(), 2);
    assert_eq!(shadows[0].namespace().as_str(), "a");
    assert_eq!(shadows[1].namespace().as_str(), "b");
}

#[test]
fn merged_path_has_no_shadows() {
    let candidates = vec![
        cand("base", ".mcp.json", "mcp"),
        cand("leaf", ".mcp.json", "mcp"),
    ];
    let shadows = compute_shadows(
        &candidates,
        MaterializeStrategy::DeepMerge(aenv_core::resolve::DeepMergeFormat::Json),
        &cc_with_instructions(),
    )
    .unwrap();
    assert!(shadows.is_empty());
}

#[test]
fn section_merged_path_has_no_shadows() {
    let candidates = vec![
        cand("base", "CLAUDE.md", "claude-code"),
        cand("leaf", "CLAUDE.md", "claude-code"),
    ];
    let shadows = compute_shadows(
        &candidates,
        MaterializeStrategy::SectionMerge,
        &cc_with_instructions(),
    )
    .unwrap();
    assert!(shadows.is_empty());
}

#[test]
fn single_candidate_has_no_shadows() {
    let shadows = compute_shadows(
        &[cand("base", "CLAUDE.md", "claude-code")],
        MaterializeStrategy::Symlink,
        &cc_with_instructions(),
    )
    .unwrap();
    assert!(shadows.is_empty());
}
