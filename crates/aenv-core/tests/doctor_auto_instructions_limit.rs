use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::doctor::evaluate;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::OutcomeStatus;
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

#[test]
fn auto_fires_when_manifest_silent_and_oversized() {
    let fs = MockFilesystem::new();
    let body = "x".repeat(8000);
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
        parameters: BTreeMap::new(),
        // CRITICAL: no policy declared. R-24 says we should still warn.
        policies: BTreeMap::new(),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    // Expect a Warn outcome from the synthesized policy.
    let warns: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
        .collect();
    assert!(
        !warns.is_empty(),
        "expected R-24 auto-warn for 8000-char file; got {:?}",
        report.outcomes
    );
    // Synthesized policy appears in the report.
    assert!(
        report.policies.contains_key("instructions_max_chars"),
        "expected synthesized instructions_max_chars in report.policies"
    );
}

#[test]
fn does_not_fire_when_manifest_declares_explicitly() {
    use aenv_core::policies::{PolicyValue, ResolvedPolicy};
    let fs = MockFilesystem::new();
    let body = "x".repeat(8000);
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

    let mut policies = BTreeMap::new();
    policies.insert(
        "instructions_max_chars".into(),
        ResolvedPolicy {
            value: PolicyValue::Integer(10_000), // looser than adapter default
            enforce: false,
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
        parameters: BTreeMap::new(),
        policies,
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    // The manifest-declared 10_000 limit means 8000 is fine.
    let warns: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
        .collect();
    assert!(
        warns.is_empty(),
        "manifest's 10_000 limit takes precedence; expected no warn; got {:?}",
        report.outcomes
    );
}

#[test]
fn does_not_fire_when_no_instructions_role_present() {
    let fs = MockFilesystem::new();
    let adapters = AdapterRegistry::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
    };
    let report = evaluate(&fs, &layout, &adapters, &resolved);
    assert!(
        !report.policies.contains_key("instructions_max_chars"),
        "no instructions files → no auto-fire"
    );
}
