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

fn make_adapters() -> AdapterRegistry {
    let mut adapters = AdapterRegistry::new();
    let mut roles = BTreeMap::new();
    roles.insert(".mcp.json".into(), "mcp".into());
    adapters.insert(Adapter {
        name: "mcp".into(),
        files: vec![".mcp.json".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
    });
    adapters
}

fn make_resolved(ns_name: &str, source: PathBuf) -> ResolutionResult {
    ResolutionResult {
        chain: vec![ns(ns_name)],
        candidates: vec![Candidate {
            namespace: ns(ns_name),
            path: PathBuf::from(".mcp.json"),
            source_path: source,
            adapter: "mcp".into(),
            merge_override: None,
        }],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
    }
}

fn enforce_true() -> ResolvedPolicy {
    ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: true,
        source: ns("base"),
    }
}

fn advisory() -> ResolvedPolicy {
    ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    }
}

#[test]
fn pass_when_command_present() {
    let fs = MockFilesystem::new();
    let body = br#"{"mcpServers":{"fs":{"command":"npx fs-mcp"}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn pass_when_url_present() {
    let fs = MockFilesystem::new();
    let body = br#"{"mcpServers":{"net":{"url":"https://example/mcp"}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn warn_when_neither_advisory() {
    let fs = MockFilesystem::new();
    let body = br#"{"mcpServers":{"broken":{"timeout":30}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    if let OutcomeStatus::Warn { msg } = &out[0].status {
        assert!(msg.contains("broken"));
        assert!(msg.contains("command") || msg.contains("url"));
    } else {
        panic!("expected Warn, got {:?}", out[0].status);
    }
}

#[test]
fn fail_when_neither_enforced() {
    let fs = MockFilesystem::new();
    let body = br#"{"mcpServers":{"broken":{"timeout":30}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &enforce_true(), &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Fail { .. }));
}

#[test]
fn warn_skip_when_json_invalid() {
    let fs = MockFilesystem::new();
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), b"not json")
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::WarnSkip { .. }));
}

#[test]
fn skips_non_mcp_files() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles: BTreeMap::new(),
        default_merge: BTreeMap::new(),
        parameters: vec![],
    });
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
        }],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
    assert!(
        out[0].target.is_none(),
        "expected targetless Pass when no MCP files match; got {out:?}"
    );
}

#[test]
fn accepts_servers_root_alias() {
    // Some configs use the bare `servers` key instead of `mcpServers`.
    let fs = MockFilesystem::new();
    let body = br#"{"servers":{"fs":{"command":"npx fs-mcp"}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn disabled_when_false() {
    let fs = MockFilesystem::new();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
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
    let out = dispatch("mcp_requires_command_or_url", &rp, &ctx);
    assert!(out.is_empty());
}
