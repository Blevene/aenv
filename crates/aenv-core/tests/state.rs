//! Tests for ActivationState serialization.

use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::state::{ActivationState, BackedUpFile, ManagedFile, MaterializeStrategy};
use aenv_core::AenvError;
use std::path::PathBuf;

fn qn(ns: &str, short: &str) -> QualifiedName {
    let nsid = if ns == NamespaceId::RESERVED_MERGED {
        NamespaceId::merged_synthetic()
    } else {
        NamespaceId::new(ns).unwrap()
    };
    QualifiedName::new(nsid, ShortName::new(short).unwrap())
}

fn sample_state() -> ActivationState {
    ActivationState {
        schema_version: 2,
        active_namespace: "experiments".to_string(),
        project_root: PathBuf::from("/projects/p"),
        managed_files: vec![ManagedFile {
            path: PathBuf::from("CLAUDE.md"),
            qualified_name: qn("experiments", "CLAUDE.md"),
            strategy: MaterializeStrategy::Symlink,
            contributors: vec![],
            shadows: vec![],
        }],
        backed_up: vec![BackedUpFile {
            original_path: PathBuf::from("CLAUDE.md"),
            backup_path: PathBuf::from(".aenv-state/backup/2026-05-20T14-22-03/CLAUDE.md"),
        }],
    }
}

#[test]
fn round_trip_via_json() {
    let state = sample_state();
    let json = state.to_json().unwrap();
    let parsed = ActivationState::from_json(&json).unwrap();
    assert_eq!(parsed, state);
}

#[test]
fn rejects_unknown_higher_schema_version() {
    let json = r#"{
        "schema_version": 999,
        "active_namespace": "x",
        "project_root": "/p",
        "managed_files": [],
        "backed_up": []
    }"#;
    let err = ActivationState::from_json(json).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
    assert!(err.to_string().contains("schema_version"));
}

#[test]
fn rejects_malformed_json() {
    let err = ActivationState::from_json("{ not json").expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn empty_state_has_no_managed_or_backed_up() {
    let json = r#"{
        "schema_version": 1,
        "active_namespace": "empty",
        "project_root": "/p",
        "managed_files": [],
        "backed_up": []
    }"#;
    let state = ActivationState::from_json(json).unwrap();
    assert_eq!(state.managed_files.len(), 0);
    assert_eq!(state.backed_up.len(), 0);
}

#[test]
fn serializes_strategy_as_lowercase_string() {
    let state = sample_state();
    let json = state.to_json().unwrap();
    assert!(json.contains("\"symlink\""));
}

// ---- Phase 2 / schema-2 tests ----

#[test]
fn schema_version_is_2_for_new_states() {
    let s = ActivationState {
        schema_version: 2,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![],
        backed_up: vec![],
    };
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"schema_version\":2"));
}

#[test]
fn managed_file_serializes_qualified_name_and_shadows() {
    let mf = ManagedFile {
        path: PathBuf::from("CLAUDE.md"),
        qualified_name: qn("leaf", "CLAUDE.md"),
        strategy: MaterializeStrategy::Symlink,
        contributors: vec![],
        shadows: vec![qn("base", "CLAUDE.md")],
    };
    let json = serde_json::to_string(&mf).unwrap();
    assert!(json.contains("\"qualified_name\""));
    assert!(json.contains("leaf::CLAUDE.md"));
    assert!(json.contains("base::CLAUDE.md"));
}

#[test]
fn managed_file_serializes_contributors_for_merged() {
    let mf = ManagedFile {
        path: PathBuf::from(".mcp.json"),
        qualified_name: qn("(merged)", ".mcp.json"),
        strategy: MaterializeStrategy::DeepMerge(
            aenv_core::resolve::DeepMergeFormat::Json,
        ),
        contributors: vec![qn("base", ".mcp.json"), qn("leaf", ".mcp.json")],
        shadows: vec![],
    };
    let json = serde_json::to_string(&mf).unwrap();
    assert!(json.contains("\"contributors\""));
    assert!(json.contains("base::.mcp.json"));
    assert!(json.contains("leaf::.mcp.json"));
}

#[test]
fn schema_1_files_load_with_empty_new_fields() {
    // Schema-1 ManagedFile only has path + strategy (Phase 1).
    // Phase 1 wrote MaterializeStrategy with lowercase serde rename,
    // so the on-disk form is "symlink" (lowercase). The new resolve.rs
    // enum also uses kebab-case (which is lowercase for single-word variants).
    let schema_1 = serde_json::json!({
        "schema_version": 1,
        "active_namespace": "base",
        "project_root": "/p",
        "managed_files": [
            { "path": "CLAUDE.md", "strategy": "symlink" }
        ],
        "backed_up": []
    });
    let s: ActivationState = serde_json::from_value(schema_1).unwrap();
    assert_eq!(s.schema_version, 1);
    let mf = &s.managed_files[0];
    assert!(mf.contributors.is_empty());
    assert!(mf.shadows.is_empty());
    // qualified_name is synthesized as <namespace>::<path>.
    assert_eq!(format!("{}", mf.qualified_name), "base::CLAUDE.md");
}
