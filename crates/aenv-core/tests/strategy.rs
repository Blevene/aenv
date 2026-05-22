use std::path::PathBuf;

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::{Candidate, DeepMergeFormat, MaterializeStrategy};
use aenv_core::strategy::decide_strategy;

fn cand(ns: &str, path: &str, adapter: &str, override_: Option<&str>) -> Candidate {
    Candidate {
        namespace: NamespaceId::new(ns).unwrap(),
        path: PathBuf::from(path),
        source_path: PathBuf::from(format!("/aenv/envs/{ns}/{path}")),
        adapter: adapter.to_string(),
        merge_override: override_.map(|s| s.to_string()),
        skill_provenance: None,
    }
}

fn cc() -> Adapter {
    toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"#,
    )
    .unwrap()
}

fn mcp() -> Adapter {
    toml::from_str(
        r#"
name = "mcp"
files = [".mcp.json"]
[default_merge]
".mcp.json" = "deep"
"#,
    )
    .unwrap()
}

fn registry() -> AdapterRegistry {
    let mut r = AdapterRegistry::default();
    r.insert(cc());
    r.insert(mcp());
    r
}

#[test]
fn single_candidate_is_symlink() {
    let strat = decide_strategy(
        &[cand("base", "CLAUDE.md", "claude-code", None)],
        &registry(),
    )
    .unwrap();
    assert!(matches!(strat, MaterializeStrategy::Symlink));
}

#[test]
fn instructions_role_with_two_candidates_section_merges() {
    let candidates = [
        cand("base", "CLAUDE.md", "claude-code", None),
        cand("leaf", "CLAUDE.md", "claude-code", None),
    ];
    let strat = decide_strategy(&candidates, &registry()).unwrap();
    assert!(matches!(strat, MaterializeStrategy::SectionMerge));
}

#[test]
fn manifest_override_wins_over_role_default() {
    let candidates = [
        cand("base", "CLAUDE.md", "claude-code", None),
        cand("leaf", "CLAUDE.md", "claude-code", Some("last-wins")),
    ];
    let strat = decide_strategy(&candidates, &registry()).unwrap();
    assert!(matches!(strat, MaterializeStrategy::Symlink));
}

#[test]
fn default_merge_deep_picks_deepjson_for_dot_mcp_json() {
    let candidates = [
        cand("base", ".mcp.json", "mcp", None),
        cand("leaf", ".mcp.json", "mcp", None),
    ];
    let strat = decide_strategy(&candidates, &registry()).unwrap();
    assert!(matches!(
        strat,
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Json)
    ));
}

#[test]
fn deep_override_on_yaml_picks_yaml_format() {
    let candidates = [
        cand("base", ".aider.conf.yml", "aider", Some("deep")),
        cand("leaf", ".aider.conf.yml", "aider", Some("deep")),
    ];
    let mut reg = registry();
    reg.insert(toml::from_str(r#"name = "aider""#).unwrap());
    let strat = decide_strategy(&candidates, &reg).unwrap();
    assert!(matches!(
        strat,
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Yaml)
    ));
}

#[test]
fn deep_override_on_toml_picks_toml_format() {
    let candidates = [
        cand("base", "config.toml", "x", Some("deep")),
        cand("leaf", "config.toml", "x", Some("deep")),
    ];
    let mut reg = AdapterRegistry::default();
    reg.insert(toml::from_str(r#"name = "x""#).unwrap());
    let strat = decide_strategy(&candidates, &reg).unwrap();
    assert!(matches!(
        strat,
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Toml)
    ));
}

#[test]
fn unknown_extension_with_deep_override_errors() {
    let candidates = [
        cand("base", "config.xyz", "x", Some("deep")),
        cand("leaf", "config.xyz", "x", Some("deep")),
    ];
    let mut reg = AdapterRegistry::default();
    reg.insert(toml::from_str(r#"name = "x""#).unwrap());
    let err = decide_strategy(&candidates, &reg).unwrap_err();
    assert!(err.to_string().contains("deep-merge requires"));
}

#[test]
fn two_candidates_no_role_no_override_fall_back_to_last_wins() {
    let candidates = [
        cand("base", ".cursorrules", "cursor", None),
        cand("leaf", ".cursorrules", "cursor", None),
    ];
    let mut reg = registry();
    reg.insert(
        toml::from_str(
            r#"name = "cursor"
files = [".cursorrules"]"#,
        )
        .unwrap(),
    );
    let strat = decide_strategy(&candidates, &reg).unwrap();
    assert!(matches!(strat, MaterializeStrategy::Symlink));
}
