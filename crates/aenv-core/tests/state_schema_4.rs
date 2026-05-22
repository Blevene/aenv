use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::MaterializeStrategy;
use aenv_core::state::{ActivationState, ManagedFile, SkillProvenance, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn schema_version_is_4() {
    assert_eq!(SCHEMA_VERSION, 4);
}

#[test]
fn schema_4_roundtrips_with_skill_provenance() {
    let qn = QualifiedName::new(
        NamespaceId::new("base").unwrap(),
        ShortName::new(".claude/skills/x/SKILL.md").unwrap(),
    );
    let state = ActivationState {
        schema_version: 4,
        active_namespace: "base".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![ManagedFile {
            path: PathBuf::from(".claude/skills/x/SKILL.md"),
            qualified_name: qn,
            strategy: MaterializeStrategy::Symlink,
            contributors: vec![],
            shadows: vec![],
            skill_provenance: Some(SkillProvenance {
                source: "/external/x".into(),
                resolved_ref: None,
                resolved_hash: "sha256:abc".into(),
            }),
        }],
        backed_up: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
    };
    let s = state.to_json().unwrap();
    let parsed = ActivationState::from_json(&s).unwrap();
    assert_eq!(parsed, state);
}

#[test]
fn reads_schema_3_with_no_skill_provenance() {
    let json = r#"{
        "schema_version": 3,
        "active_namespace": "base",
        "project_root": "/p",
        "managed_files": [{
            "path": "CLAUDE.md",
            "qualified_name": "base::CLAUDE.md",
            "strategy": "symlink",
            "contributors": [],
            "shadows": []
        }],
        "backed_up": [],
        "parameters": {},
        "policies": {}
    }"#;
    let parsed = ActivationState::from_json(json).unwrap();
    assert_eq!(parsed.schema_version, 3);
    assert!(parsed.managed_files[0].skill_provenance.is_none());
}
