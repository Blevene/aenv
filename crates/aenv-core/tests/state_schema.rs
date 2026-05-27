use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::parameters::{ParameterValue, ResolvedParameter};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::MaterializeStrategy;
use aenv_core::state::{ActivationState, ManagedFile, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn schema_version_is_5() {
    assert_eq!(SCHEMA_VERSION, 5);
}

#[test]
fn schema_3_roundtrip_with_params_and_policies() {
    let qn = QualifiedName::new(
        NamespaceId::new("base").unwrap(),
        ShortName::new("CLAUDE.md").unwrap(),
    );
    let mut parameters = BTreeMap::new();
    parameters.insert(
        "default_model".into(),
        ResolvedParameter {
            value: ParameterValue::String("opus".into()),
            source: NamespaceId::new("leaf").unwrap(),
        },
    );
    let mut policies = BTreeMap::new();
    policies.insert(
        "instructions_max_chars".into(),
        ResolvedPolicy {
            value: PolicyValue::Integer(3000),
            enforce: true,
            source: NamespaceId::new("leaf").unwrap(),
        },
    );
    let state = ActivationState {
        schema_version: 3,
        active_namespace: "leaf".into(),
        scope: aenv_core::scope::Scope::Project,
        project_root: PathBuf::from("/p"),
        managed_files: vec![ManagedFile {
            path: PathBuf::from("CLAUDE.md"),
            qualified_name: qn,
            strategy: MaterializeStrategy::Symlink,
            contributors: vec![],
            shadows: vec![],
            skill_provenance: None,
        }],
        backed_up: vec![],
        parameters,
        policies,
        warnings: Vec::new(),
    };
    let s = state.to_json().unwrap();
    let parsed = ActivationState::from_json(&s).unwrap();
    assert_eq!(parsed, state);
}

#[test]
fn reads_schema_2_with_default_empty_maps() {
    // A schema-2 state file: no parameters, no policies fields.
    let json = r#"{
        "schema_version": 2,
        "active_namespace": "base",
        "project_root": "/p",
        "managed_files": [],
        "backed_up": []
    }"#;
    let s = ActivationState::from_json(json).unwrap();
    assert_eq!(s.schema_version, 2);
    assert!(s.parameters.is_empty());
    assert!(s.policies.is_empty());
}

#[test]
fn rejects_unknown_higher_schema_version() {
    let json = r#"{
        "schema_version": 6,
        "active_namespace": "base",
        "project_root": "/p",
        "managed_files": [],
        "backed_up": []
    }"#;
    let err = ActivationState::from_json(json).unwrap_err();
    assert!(err.to_string().contains("schema_version 6"));
}
