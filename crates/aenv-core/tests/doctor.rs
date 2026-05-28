use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::doctor::{evaluate, DoctorReport};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::OutcomeStatus;
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn claude_adapter() -> Adapter {
    let mut roles = BTreeMap::new();
    roles.insert("CLAUDE.md".into(), "instructions".into());
    Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
        skills_dir: None,
        soft_limits: BTreeMap::new(),
        user_files: vec![],
        user_roles: BTreeMap::new(),
        user_default_merge: BTreeMap::new(),
        user_merge_strategies: BTreeMap::new(),
        user_soft_limits: BTreeMap::new(),
        user_skills_dir: None,
        materialize: None,
    }
}

#[test]
fn clean_report_when_all_pass() {
    let fs = MockFilesystem::new();
    fs.write(
        &PathBuf::from("/h/envs/base/CLAUDE.md"),
        "small body".as_bytes(),
    )
    .unwrap();
    let mut adapters = AdapterRegistry::new();
    adapters.insert(claude_adapter());
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            scope: aenv_core::scope::Scope::Project,
            merge_override: None,
            skill_provenance: None,
            adapter_materialize_override: None,
        }],
        parameters: BTreeMap::new(),
        policies: BTreeMap::from([(
            "instructions_max_chars".into(),
            ResolvedPolicy {
                value: PolicyValue::Integer(5000),
                enforce: false,
                source: ns("base"),
            },
        )]),
        warnings: Vec::new(),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved, &PathBuf::from("/h"));
    assert!(!report.has_enforce_violations());
    assert_eq!(report.fail_count(), 0);
    assert!(report
        .outcomes
        .iter()
        .any(|o| matches!(o.status, OutcomeStatus::Pass)));
}

#[test]
fn enforce_violation_is_flagged() {
    let fs = MockFilesystem::new();
    fs.write(
        &PathBuf::from("/h/envs/base/CLAUDE.md"),
        "x".repeat(10000).as_bytes(),
    )
    .unwrap();
    let mut adapters = AdapterRegistry::new();
    adapters.insert(claude_adapter());
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            scope: aenv_core::scope::Scope::Project,
            merge_override: None,
            skill_provenance: None,
            adapter_materialize_override: None,
        }],
        parameters: BTreeMap::new(),
        policies: BTreeMap::from([(
            "instructions_max_chars".into(),
            ResolvedPolicy {
                value: PolicyValue::Integer(5000),
                enforce: true,
                source: ns("base"),
            },
        )]),
        warnings: Vec::new(),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved, &PathBuf::from("/h"));
    assert!(report.has_enforce_violations());
    assert_eq!(report.fail_count(), 1);
    let summary = report.summary_line();
    assert!(summary.contains("1 enforce") || summary.contains("violation"));
}

#[test]
fn empty_policies_means_pass() {
    let fs = MockFilesystem::new();
    let adapters = AdapterRegistry::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved, &PathBuf::from("/h"));
    assert!(!report.has_enforce_violations());
    assert!(report.outcomes.is_empty());
}

#[test]
fn report_records_chain_and_namespace_count() {
    let fs = MockFilesystem::new();
    let adapters = AdapterRegistry::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let resolved = ResolutionResult {
        chain: vec![ns("base"), ns("leaf")],
        candidates: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let report: DoctorReport = evaluate(&fs, &layout, &adapters, &resolved, &PathBuf::from("/h"));
    assert_eq!(report.chain.len(), 2);
    assert_eq!(report.chain[1].as_str(), "leaf");
}
