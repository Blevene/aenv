use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::doctor::evaluate;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::parameters::{ParameterValue, ResolvedParameter};
use aenv_core::policies::builtin::OutcomeStatus;
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

#[test]
fn instructions_budget_narrows_effective_limit() {
    let fs = MockFilesystem::new();
    let body = "x".repeat(4000);
    fs.write(&PathBuf::from("/h/envs/base/CLAUDE.md"), body.as_bytes())
        .unwrap();

    let mut adapters = AdapterRegistry::new();
    let mut roles = BTreeMap::new();
    roles.insert("CLAUDE.md".into(), "instructions".into());
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
        skills_dir: Some(".claude/skills".into()),
        soft_limits: BTreeMap::from([("instructions".into(), 5000usize)]),
    });
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    let mut parameters: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    parameters.insert(
        "instructions_budget".into(),
        ResolvedParameter {
            value: ParameterValue::Integer(3000),
            source: ns("base"),
        },
    );

    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
            skill_provenance: None,
        }],
        parameters,
        policies: BTreeMap::from([(
            "instructions_max_chars".into(),
            ResolvedPolicy {
                value: PolicyValue::Integer(5000),
                enforce: false,
                source: ns("base"),
            },
        )]),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    // 4000 chars > 3000 effective limit (budget narrows from 5000 to 3000).
    let fails: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
        .collect();
    assert!(
        !fails.is_empty(),
        "expected a warning (effective limit=3000, body=4000); got {:?}",
        report.outcomes
    );
}
