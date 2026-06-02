//! Unit tests for `scaffold_global_namespace` — the from-scratch user-scope
//! namespace scaffolder behind `aenv global new`.

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::error::AenvError;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::global_snapshot::scaffold_global_namespace;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

fn claude_adapter() -> Adapter {
    let mut user_roles = BTreeMap::new();
    user_roles.insert(
        "~/.claude/CLAUDE.md".to_string(),
        "instructions".to_string(),
    );
    Adapter {
        name: "claude-code".to_string(),
        files: Vec::new(),
        merge_strategies: Default::default(),
        roles: Default::default(),
        default_merge: Default::default(),
        parameters: Vec::new(),
        skills_dir: None,
        soft_limits: Default::default(),
        user_files: vec![
            "~/.claude/CLAUDE.md".to_string(),
            "~/.claude/agents/".to_string(),
        ],
        user_roles,
        user_default_merge: Default::default(),
        user_merge_strategies: Default::default(),
        user_soft_limits: Default::default(),
        user_skills_dir: None,
        materialize: None,
    }
}

fn registry() -> AdapterRegistry {
    let mut reg = AdapterRegistry::new();
    reg.insert(claude_adapter());
    reg
}

#[test]
fn scaffold_seeds_instructions_file_and_declares_it() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let reg = registry();

    let summary =
        scaffold_global_namespace(&fs, &layout, &reg, "mine", "claude-code", false).unwrap();

    assert_eq!(
        summary.seeded_instructions.as_deref(),
        Some(".claude/CLAUDE.md")
    );
    assert_eq!(
        summary.user_files_declared,
        vec![".claude/CLAUDE.md".to_string()]
    );

    // The seeded file exists under user/ with the starter header.
    let seeded = layout.namespace_dir("mine").join("user/.claude/CLAUDE.md");
    let body = fs.read(&seeded).unwrap();
    assert_eq!(String::from_utf8(body).unwrap(), "# mine\n");

    // The manifest declares the user file under the claude-code adapter.
    let manifest_bytes = fs.read(&layout.manifest_path("mine")).unwrap();
    let manifest = AenvManifest::from_toml(&String::from_utf8(manifest_bytes).unwrap()).unwrap();
    let entry = manifest.adapters.get("claude-code").expect("adapter block");
    assert_eq!(entry.user_files, vec![".claude/CLAUDE.md".to_string()]);
}

#[test]
fn scaffold_refuses_existing_namespace() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let reg = registry();
    scaffold_global_namespace(&fs, &layout, &reg, "dup", "claude-code", false).unwrap();
    let err =
        scaffold_global_namespace(&fs, &layout, &reg, "dup", "claude-code", false).unwrap_err();
    assert!(matches!(err, AenvError::ActivationConflict(_)));
}

#[test]
fn scaffold_unknown_adapter_errors() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let reg = registry();
    let err = scaffold_global_namespace(&fs, &layout, &reg, "x", "nope", false).unwrap_err();
    assert!(matches!(err, AenvError::AdapterMissing(_)));
}
