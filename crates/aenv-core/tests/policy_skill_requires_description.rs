use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::{Filesystem, MockFilesystem};
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

fn make_registry() -> AdapterRegistry {
    let mut adapters = AdapterRegistry::new();
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec![".claude/".into()],
        merge_strategies: BTreeMap::new(),
        roles: BTreeMap::new(),
        default_merge: BTreeMap::new(),
        parameters: vec![],
        skills_dir: None,
        soft_limits: BTreeMap::new(),
    });
    adapters
}

fn make_layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv-home"))
}

fn skill_candidate(ns_name: &str, skill: &str, source: PathBuf) -> Candidate {
    Candidate {
        namespace: ns(ns_name),
        path: PathBuf::from(format!(".claude/skills/{skill}/SKILL.md")),
        source_path: source,
        adapter: "claude-code".into(),
        merge_override: None,
        skill_provenance: None,
    }
}

#[test]
fn pass_when_description_present() {
    let fs = MockFilesystem::new();
    let body = "---\nname: write-tests\ndescription: Writes tests for changed code\n---\nBody";
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/.claude/skills/write-tests/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![skill_candidate(
            "base",
            "write-tests",
            PathBuf::from("/aenv-home/envs/base/.claude/skills/write-tests/SKILL.md"),
        )],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn warn_when_description_missing() {
    let fs = MockFilesystem::new();
    let body = "---\nname: half-baked\n---\nBody";
    fs.write(
        &PathBuf::from("/aenv-home/envs/x/.claude/skills/half-baked/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("x")],
        candidates: vec![skill_candidate(
            "x",
            "half-baked",
            PathBuf::from("/aenv-home/envs/x/.claude/skills/half-baked/SKILL.md"),
        )],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    if let OutcomeStatus::Warn { msg } = &out[0].status {
        assert!(msg.contains("description"));
        assert!(msg.contains("half-baked"));
    } else {
        panic!("expected Warn, got {:?}", out[0].status);
    }
}

#[test]
fn fail_when_enforced_and_description_missing() {
    let fs = MockFilesystem::new();
    let body = "---\nname: half-baked\n---\nBody";
    fs.write(
        &PathBuf::from("/aenv-home/envs/x/.claude/skills/half-baked/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("x")],
        candidates: vec![skill_candidate(
            "x",
            "half-baked",
            PathBuf::from("/aenv-home/envs/x/.claude/skills/half-baked/SKILL.md"),
        )],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: true,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Fail { .. }));
}

#[test]
fn fail_when_description_empty() {
    let fs = MockFilesystem::new();
    let body = "---\nname: x\ndescription:   \n---\nBody";
    fs.write(
        &PathBuf::from("/aenv-home/envs/x/.claude/skills/x/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("x")],
        candidates: vec![skill_candidate(
            "x",
            "x",
            PathBuf::from("/aenv-home/envs/x/.claude/skills/x/SKILL.md"),
        )],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Warn { .. }));
}

#[test]
fn warn_when_no_frontmatter_at_all() {
    let fs = MockFilesystem::new();
    let body = "no frontmatter here";
    fs.write(
        &PathBuf::from("/aenv-home/envs/x/.claude/skills/raw/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("x")],
        candidates: vec![skill_candidate(
            "x",
            "raw",
            PathBuf::from("/aenv-home/envs/x/.claude/skills/raw/SKILL.md"),
        )],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Warn { .. }));
}

#[test]
fn skips_non_skill_files() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = make_registry();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/aenv-home/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
            skill_provenance: None,
        }],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
    assert!(
        out[0].target.is_none(),
        "expected targetless Pass when no skill files match; got {out:?}"
    );
}

#[test]
fn disabled_when_false() {
    let fs = MockFilesystem::new();
    // No body even needed — the policy is off.
    let layout = make_layout();
    let adapters = make_registry();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![skill_candidate(
            "base",
            "x",
            PathBuf::from("/aenv-home/envs/base/.claude/skills/x/SKILL.md"),
        )],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        warnings: Vec::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(false),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    // When value is false the policy is off; no outcomes.
    assert!(out.is_empty());
}
