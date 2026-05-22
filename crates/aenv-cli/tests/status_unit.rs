use std::path::PathBuf;

use aenv_cli::cmd::status::format_status;
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::DeepMergeFormat;
use aenv_core::state::{ActivationState, ManagedFile, MaterializeStrategy};

fn qn(ns: &str, short: &str) -> QualifiedName {
    let nsid = if ns == NamespaceId::RESERVED_MERGED {
        NamespaceId::merged_synthetic()
    } else {
        NamespaceId::new(ns).unwrap()
    };
    QualifiedName::new(nsid, ShortName::new(short).unwrap())
}

#[test]
fn status_prints_resolution_chain_and_managed_provenance() {
    let state = ActivationState {
        schema_version: 2,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![
            ManagedFile {
                path: PathBuf::from("CLAUDE.md"),
                qualified_name: qn("(merged)", "CLAUDE.md"),
                strategy: MaterializeStrategy::SectionMerge,
                contributors: vec![qn("base", "CLAUDE.md"), qn("leaf", "CLAUDE.md")],
                shadows: vec![],
                skill_provenance: None,
            },
            ManagedFile {
                path: PathBuf::from(".claude/skills/write-tests/SKILL.md"),
                qualified_name: qn("leaf", ".claude/skills/write-tests/SKILL.md"),
                strategy: MaterializeStrategy::Symlink,
                contributors: vec![],
                shadows: vec![qn("base", ".claude/skills/write-tests/SKILL.md")],
                skill_provenance: None,
            },
            ManagedFile {
                path: PathBuf::from(".mcp.json"),
                qualified_name: qn("(merged)", ".mcp.json"),
                strategy: MaterializeStrategy::DeepMerge(DeepMergeFormat::Json),
                contributors: vec![qn("base", ".mcp.json"), qn("leaf", ".mcp.json")],
                shadows: vec![],
                skill_provenance: None,
            },
        ],
        backed_up: vec![],
        parameters: std::collections::BTreeMap::new(),
        policies: std::collections::BTreeMap::new(),
    };
    let chain = vec![
        NamespaceId::new("base").unwrap(),
        NamespaceId::new("leaf").unwrap(),
    ];
    let out = format_status(&state, &chain);
    assert!(out.contains("Active namespace: leaf"));
    assert!(out.contains("Resolution:       base → leaf"));
    assert!(out.contains("CLAUDE.md"));
    assert!(out.contains("merged from base + leaf"));
    assert!(out.contains("write-tests"));
    assert!(out.contains("(shadows base::"));
    assert!(out.contains(".mcp.json"));
    assert!(out.contains("merged (deep-merge json) from base + leaf"));
}

#[test]
fn status_no_active_namespace() {
    let state = ActivationState {
        schema_version: 2,
        active_namespace: "alone".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![],
        backed_up: vec![],
        parameters: std::collections::BTreeMap::new(),
        policies: std::collections::BTreeMap::new(),
    };
    let chain = vec![NamespaceId::new("alone").unwrap()];
    let out = format_status(&state, &chain);
    assert!(out.contains("Resolution:       alone"));
    assert!(out.contains("No managed files."));
}
