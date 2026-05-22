use std::path::{Path, PathBuf};

use aenv_cli::cmd::which::format_which;
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::MaterializeStrategy;
use aenv_core::state::{ActivationState, ManagedFile};

fn qn(ns: &str, short: &str) -> QualifiedName {
    let nsid = if ns == NamespaceId::RESERVED_MERGED {
        NamespaceId::merged_synthetic()
    } else {
        NamespaceId::new(ns).unwrap()
    };
    QualifiedName::new(nsid, ShortName::new(short).unwrap())
}

fn state_with(mf: ManagedFile) -> ActivationState {
    ActivationState {
        schema_version: 2,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![mf],
        backed_up: vec![],
        parameters: std::collections::BTreeMap::new(),
        policies: std::collections::BTreeMap::new(),
    }
}

#[test]
fn which_for_symlinked_file_with_shadow() {
    let state = state_with(ManagedFile {
        path: PathBuf::from("CLAUDE.md"),
        qualified_name: qn("leaf", "CLAUDE.md"),
        strategy: MaterializeStrategy::Symlink,
        contributors: vec![],
        shadows: vec![qn("base", "CLAUDE.md")],
        skill_provenance: None,
    });
    let out = format_which(&state, Path::new("CLAUDE.md")).unwrap();
    assert!(out.contains("Qualified name:  leaf::CLAUDE.md"));
    assert!(out.contains("Strategy:        symlink"));
    assert!(out.contains("Shadows:"));
    assert!(out.contains("base::CLAUDE.md"));
}

#[test]
fn which_for_merged_file_lists_contributors() {
    let state = state_with(ManagedFile {
        path: PathBuf::from(".mcp.json"),
        qualified_name: qn("(merged)", ".mcp.json"),
        strategy: MaterializeStrategy::DeepMerge(aenv_core::resolve::DeepMergeFormat::Json),
        contributors: vec![qn("base", ".mcp.json"), qn("leaf", ".mcp.json")],
        shadows: vec![],
        skill_provenance: None,
    });
    let out = format_which(&state, Path::new(".mcp.json")).unwrap();
    assert!(out.contains("Qualified name:  (merged)"));
    assert!(out.contains("Strategy:        deep-merge (json)"));
    assert!(out.contains("Contributors:"));
    assert!(out.contains("base::.mcp.json"));
    assert!(out.contains("leaf::.mcp.json"));
}

#[test]
fn which_for_unmanaged_path_reports_error() {
    let state = ActivationState {
        schema_version: 2,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![],
        backed_up: vec![],
        parameters: std::collections::BTreeMap::new(),
        policies: std::collections::BTreeMap::new(),
    };
    let err = format_which(&state, Path::new("unmanaged.txt")).unwrap_err();
    assert!(err.to_string().contains("not managed"));
}
