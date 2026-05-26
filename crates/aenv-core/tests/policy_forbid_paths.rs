use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::MockFilesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::{dispatch, OutcomeStatus, PolicyContext};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn dummy_adapter() -> Adapter {
    Adapter {
        name: "claude-code".into(),
        files: vec![],
        merge_strategies: BTreeMap::new(),
        roles: BTreeMap::new(),
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
    }
}

fn candidate(rel: &str) -> Candidate {
    Candidate {
        namespace: ns("base"),
        path: PathBuf::from(rel),
        source_path: PathBuf::from(format!("/h/envs/base/{rel}")),
        adapter: "claude-code".into(),
        scope: aenv_core::scope::Scope::Project,
        merge_override: None,
        skill_provenance: None,
    }
}

fn forbid(value: Vec<&str>, enforce: bool) -> ResolvedPolicy {
    ResolvedPolicy {
        value: PolicyValue::ListString(value.into_iter().map(String::from).collect()),
        enforce,
        source: ns("base"),
    }
}

fn ctx<'a>(
    fs: &'a MockFilesystem,
    layout: &'a RegistryLayout,
    adapters: &'a AdapterRegistry,
    resolved: &'a ResolutionResult,
) -> PolicyContext<'a, MockFilesystem> {
    PolicyContext {
        fs,
        layout,
        adapters,
        resolved,
    }
}

#[test]
fn exact_match_advisory_warns() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate(".env")],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let policy = forbid(vec![".env"], false);
    let out = dispatch(
        "forbid_paths",
        &policy,
        &ctx(&fs, &layout, &adapters, &resolved),
    );
    assert_eq!(out.len(), 1);
    if let OutcomeStatus::Warn { msg } = &out[0].status {
        assert!(msg.contains(".env"));
    } else {
        panic!("expected Warn, got {:?}", out[0].status);
    }
}

#[test]
fn exact_match_enforced_fails() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate(".env")],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let policy = forbid(vec![".env"], true);
    let out = dispatch(
        "forbid_paths",
        &policy,
        &ctx(&fs, &layout, &adapters, &resolved),
    );
    assert!(matches!(out[0].status, OutcomeStatus::Fail { .. }));
}

#[test]
fn star_suffix_matches() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate(".env.production")],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let policy = forbid(vec![".env*"], false);
    let out = dispatch(
        "forbid_paths",
        &policy,
        &ctx(&fs, &layout, &adapters, &resolved),
    );
    assert!(matches!(out[0].status, OutcomeStatus::Warn { .. }));
}

#[test]
fn glob_double_star_matches_subtree() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate("secrets/db.json")],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let policy = forbid(vec!["secrets/**"], false);
    let out = dispatch(
        "forbid_paths",
        &policy,
        &ctx(&fs, &layout, &adapters, &resolved),
    );
    assert!(matches!(out[0].status, OutcomeStatus::Warn { .. }));
}

#[test]
fn pass_outcome_when_no_match() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate("CLAUDE.md")],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let policy = forbid(vec![".env*", "secrets/**"], false);
    let out = dispatch(
        "forbid_paths",
        &policy,
        &ctx(&fs, &layout, &adapters, &resolved),
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn empty_list_passes() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate(".env")],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let policy = forbid(vec![], false);
    let out = dispatch(
        "forbid_paths",
        &policy,
        &ctx(&fs, &layout, &adapters, &resolved),
    );
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn wrong_value_type_warn_skips() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let adapters = AdapterRegistry::new();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let policy = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch(
        "forbid_paths",
        &policy,
        &ctx(&fs, &layout, &adapters, &resolved),
    );
    assert!(matches!(out[0].status, OutcomeStatus::WarnSkip { .. }));
}
