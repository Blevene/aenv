use std::path::PathBuf;

use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::{MaterializeStrategy, ResolvedArtifact, ResolvedNamespace};

fn qn(ns: &str, short: &str) -> QualifiedName {
    let nsid = if ns == NamespaceId::RESERVED_MERGED {
        NamespaceId::merged_synthetic()
    } else {
        NamespaceId::new(ns).unwrap()
    };
    QualifiedName::new(nsid, ShortName::new(short).unwrap())
}

#[test]
fn resolved_namespace_constructs() {
    let resolved = ResolvedNamespace {
        chain: vec![
            NamespaceId::new("base").unwrap(),
            NamespaceId::new("detailed-execution").unwrap(),
        ],
        artifacts: vec![],
    };
    assert_eq!(resolved.chain.len(), 2);
    assert_eq!(resolved.chain[0].as_str(), "base");
}

#[test]
fn artifact_carries_qualified_name_and_strategy() {
    let art = ResolvedArtifact {
        qualified_name: qn("detailed-execution", "write-tests"),
        materialized_path: PathBuf::from(".claude/skills/write-tests/SKILL.md"),
        source_path: PathBuf::from(
            "/home/u/.aenv/envs/detailed-execution/.claude/skills/write-tests/SKILL.md",
        ),
        strategy: MaterializeStrategy::Symlink,
        shadows: vec![qn("base", "write-tests")],
        contributors: vec![],
    };
    assert_eq!(
        art.qualified_name.namespace().as_str(),
        "detailed-execution"
    );
    assert!(matches!(art.strategy, MaterializeStrategy::Symlink));
    assert_eq!(art.shadows.len(), 1);
}

#[test]
fn strategy_supports_three_merge_kinds() {
    use MaterializeStrategy::*;
    let _ = Symlink;
    let _ = Identical;
    let _ = SectionMerge;
    let _ = DeepMerge(aenv_core::resolve::DeepMergeFormat::Json);
    let _ = DeepMerge(aenv_core::resolve::DeepMergeFormat::Yaml);
    let _ = DeepMerge(aenv_core::resolve::DeepMergeFormat::Toml);
}

#[test]
fn merged_artifact_has_contributors_no_shadows() {
    let art = ResolvedArtifact {
        qualified_name: qn("(merged)", ".mcp.json"),
        materialized_path: PathBuf::from(".mcp.json"),
        source_path: PathBuf::new(), // unused for merged
        strategy: MaterializeStrategy::DeepMerge(aenv_core::resolve::DeepMergeFormat::Json),
        shadows: vec![],
        contributors: vec![qn("base", ".mcp.json"), qn("leaf", ".mcp.json")],
    };
    assert!(art.shadows.is_empty());
    assert_eq!(art.contributors.len(), 2);
}
