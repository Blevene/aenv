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

fn make_registry_with_claude() -> AdapterRegistry {
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
    });
    adapters
}

fn make_layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv-home"))
}

fn make_resolution(ns_name: &str, source_path: PathBuf) -> ResolutionResult {
    ResolutionResult {
        chain: vec![ns(ns_name)],
        candidates: vec![Candidate {
            namespace: ns(ns_name),
            path: PathBuf::from("CLAUDE.md"),
            source_path,
            adapter: "claude-code".into(),
            merge_override: None,
        }],
        parameters: std::collections::BTreeMap::new(),
        policies: std::collections::BTreeMap::new(),
    }
}

#[test]
fn pass_when_under_limit() {
    let fs = MockFilesystem::new();
    fs.write(&PathBuf::from("/aenv-home/envs/base/CLAUDE.md"), b"hello")
        .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = make_resolution("base", PathBuf::from("/aenv-home/envs/base/CLAUDE.md"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };

    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5000),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(
        matches!(out[0].status, OutcomeStatus::Pass),
        "out = {out:?}"
    );
}

#[test]
fn warn_when_over_limit_and_advisory() {
    let fs = MockFilesystem::new();
    let body = "x".repeat(6000);
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = make_resolution("base", PathBuf::from("/aenv-home/envs/base/CLAUDE.md"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5000),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert_eq!(out.len(), 1);
    if let OutcomeStatus::Warn { msg } = &out[0].status {
        assert!(msg.contains("6000"));
        assert!(msg.contains("5000"));
    } else {
        panic!("expected Warn, got {:?}", out[0].status);
    }
}

#[test]
fn fail_when_over_limit_and_enforced() {
    let fs = MockFilesystem::new();
    let body = "x".repeat(6000);
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = make_resolution("base", PathBuf::from("/aenv-home/envs/base/CLAUDE.md"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5000),
        enforce: true,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Fail { .. }));
}

#[test]
fn counts_utf8_chars_not_bytes() {
    // "é" is 2 bytes but 1 char. Limit = 5 chars; body has 4 chars ("éééé").
    let fs = MockFilesystem::new();
    let body = "éééé"; // 4 chars, 8 bytes
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = make_resolution("base", PathBuf::from("/aenv-home/envs/base/CLAUDE.md"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn skips_non_instructions_files() {
    // Adapter registers CLAUDE.md but candidate path is some other file
    // not declared as `instructions` role.
    let fs = MockFilesystem::new();
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/.mcp.json"),
        b"{ \"servers\": {} }",
    )
    .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from(".mcp.json"),
            source_path: PathBuf::from("/aenv-home/envs/base/.mcp.json"),
            adapter: "claude-code".into(),
            merge_override: None,
        }],
        parameters: std::collections::BTreeMap::new(),
        policies: std::collections::BTreeMap::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5000),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert!(
        out.is_empty(),
        "expected zero outcomes when no instructions files match; got {out:?}"
    );
}

#[test]
fn wrong_value_type_warn_skips() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![],
        parameters: std::collections::BTreeMap::new(),
        policies: std::collections::BTreeMap::new(),
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
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::WarnSkip { .. }));
}
