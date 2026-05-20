//! Tests for ActivationState serialization.

use aenv_core::state::{ActivationState, BackedUpFile, ManagedFile, MaterializeStrategy};
use aenv_core::AenvError;
use std::path::PathBuf;

fn sample_state() -> ActivationState {
    ActivationState {
        schema_version: 1,
        active_namespace: "experiments".to_string(),
        project_root: PathBuf::from("/projects/p"),
        managed_files: vec![ManagedFile {
            path: PathBuf::from("CLAUDE.md"),
            strategy: MaterializeStrategy::Symlink,
            source: Some(PathBuf::from("/aenv/envs/experiments/CLAUDE.md")),
        }],
        backed_up: vec![BackedUpFile {
            original_path: PathBuf::from("CLAUDE.md"),
            backup_path: PathBuf::from(".aenv/backup/2026-05-20T14-22-03/CLAUDE.md"),
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
