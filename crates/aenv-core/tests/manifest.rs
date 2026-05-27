//! Tests for `aenv.toml` parsing.

use aenv_core::manifest::{AdapterEntry, AenvManifest};
use aenv_core::AenvError;

#[test]
fn parses_minimal_manifest_with_one_adapter() {
    let toml = r#"
        name = "experiments"

        [adapters.claude-code]
        files = ["CLAUDE.md"]
    "#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.name, "experiments");
    assert_eq!(m.extends, Vec::<String>::new());
    assert_eq!(m.adapters.len(), 1);
    let claude = m.adapters.get("claude-code").unwrap();
    assert_eq!(claude.files, vec!["CLAUDE.md".to_string()]);
}

#[test]
fn parses_extends_list_when_present() {
    let toml = r#"
        name = "detailed-execution"
        extends = ["base"]

        [adapters.claude-code]
        files = ["CLAUDE.md", ".claude/"]
    "#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.extends, vec!["base".to_string()]);
}

#[test]
fn parses_multiple_adapters() {
    let toml = r#"
        name = "experiments"

        [adapters.claude-code]
        files = ["CLAUDE.md"]

        [adapters.cursor]
        files = [".cursorrules"]
    "#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.adapters.len(), 2);
}

#[test]
fn rejects_missing_name() {
    let toml = r#"
        [adapters.claude-code]
        files = ["CLAUDE.md"]
    "#;
    let err = AenvManifest::from_toml(toml).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
    assert_eq!(err.exit_code(), 12);
}

#[test]
fn rejects_malformed_toml() {
    let toml = r#"name = "experiments" this is not valid toml"#;
    let err = AenvManifest::from_toml(toml).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn empty_adapters_table_is_valid() {
    // A namespace with no adapters declares no managed files. Valid but
    // useless; activation will just be a no-op.
    let toml = r#"name = "empty""#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.name, "empty");
    assert!(m.adapters.is_empty());
}

#[test]
fn round_trip_default_manifest() {
    // `aenv create <name>` writes a default manifest; parsing it back must
    // produce the same logical content.
    let toml = AenvManifest::default_for("experiments").to_toml();
    let m = AenvManifest::from_toml(&toml).unwrap();
    assert_eq!(m.name, "experiments");
    assert!(m.adapters.is_empty());
    assert!(m.extends.is_empty());
}

#[test]
fn adapter_entry_default_files_is_empty() {
    // Backstop: an adapter with no `files` key parses as having no files.
    let toml = r#"
        name = "experiments"

        [adapters.claude-code]
    "#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let claude = m.adapters.get("claude-code").unwrap();
    assert_eq!(claude.files, Vec::<String>::new());
}

#[test]
fn parses_per_file_merge_override() {
    let toml = r#"
name = "leaf"
extends = ["base"]
[adapters.claude-code]
files = ["CLAUDE.md", ".mcp.json"]
merge = { ".mcp.json" = "deep" }
"#;
    let m: aenv_core::manifest::AenvManifest = toml::from_str(toml).unwrap();
    let entry = m.adapters.get("claude-code").unwrap();
    assert_eq!(
        entry.merge.as_ref().unwrap().get(".mcp.json").unwrap(),
        "deep"
    );
}

#[test]
fn adapter_entry_fields_are_publicly_constructible() {
    // Compile-time check: AdapterEntry's fields stay pub. Downstream
    // consumers (Phase 2's composition layer) build these directly.
    let entry = AdapterEntry {
        files: vec!["CLAUDE.md".to_string()],
        merge: None,
        user_files: vec![],
        user_merge: None,
    };
    assert_eq!(entry.files.len(), 1);
}

#[test]
fn merge_bare_string_form_parses() {
    let toml = r#"
name = "leaf"
[adapters.mcp]
files = [".mcp.json"]
merge = "deep"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let mcp = m.adapters.get("mcp").unwrap();
    let merge = mcp.merge.as_ref().expect("merge expanded");
    assert_eq!(merge.get(".mcp.json"), Some(&"deep".to_string()));
}

#[test]
fn merge_per_file_map_form_still_parses() {
    let toml = r#"
name = "leaf"
[adapters.mcp]
files = [".mcp.json", ".aider/config.json"]
merge = { ".mcp.json" = "deep", ".aider/config.json" = "section" }
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let mcp = m.adapters.get("mcp").unwrap();
    let merge = mcp.merge.as_ref().expect("merge");
    assert_eq!(merge.get(".mcp.json"), Some(&"deep".to_string()));
    assert_eq!(
        merge.get(".aider/config.json"),
        Some(&"section".to_string())
    );
}

#[test]
fn merge_bare_string_expands_to_every_file() {
    let toml = r#"
name = "leaf"
[adapters.mcp]
files = ["a.json", "b.json", "c.json"]
merge = "deep"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let mcp = m.adapters.get("mcp").unwrap();
    let merge = mcp.merge.as_ref().unwrap();
    assert_eq!(merge.len(), 3);
    for f in &["a.json", "b.json", "c.json"] {
        assert_eq!(merge.get(*f), Some(&"deep".to_string()), "missing key {f}");
    }
}

#[test]
fn manifest_user_files_round_trip() {
    let toml = r#"
name = "research"

[adapters.claude-code]
files = ["CLAUDE.md"]
user_files = [".claude/CLAUDE.md", ".claude/agents/code-reviewer.md", ".claude/settings.json"]
user_merge = { ".claude/settings.json" = "deep" }
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    let entry = m.adapters.get("claude-code").expect("adapter present");
    assert_eq!(
        entry.user_files,
        vec![
            ".claude/CLAUDE.md".to_string(),
            ".claude/agents/code-reviewer.md".to_string(),
            ".claude/settings.json".to_string(),
        ]
    );
    let user_merge = entry.user_merge.as_ref().expect("user_merge present");
    assert_eq!(
        user_merge.get(".claude/settings.json").map(String::as_str),
        Some("deep")
    );
}

#[test]
fn manifest_user_files_optional() {
    let toml = r#"
name = "legacy"

[adapters.claude-code]
files = ["CLAUDE.md"]
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    let entry = m.adapters.get("claude-code").unwrap();
    assert!(entry.user_files.is_empty());
    assert!(entry.user_merge.is_none());
}

#[test]
fn manifest_user_files_uniform_merge() {
    let toml = r#"
name = "uniform"

[adapters.claude-code]
user_files = [".claude/a.json", ".claude/b.json"]
user_merge = "deep"
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    let entry = m.adapters.get("claude-code").unwrap();
    let user_merge = entry.user_merge.as_ref().unwrap();
    assert_eq!(
        user_merge.get(".claude/a.json").map(String::as_str),
        Some("deep")
    );
    assert_eq!(
        user_merge.get(".claude/b.json").map(String::as_str),
        Some("deep")
    );
}

// ---- [lifecycle] (Milestone K, Task 7) ----

#[test]
fn manifest_lifecycle_roundtrip() {
    let toml = r#"
name = "ns"

[lifecycle]
on_activate = "install.sh"
on_deactivate = "uninstall.sh"
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.lifecycle.on_activate.as_deref(), Some("install.sh"));
    assert_eq!(m.lifecycle.on_deactivate.as_deref(), Some("uninstall.sh"));
}

#[test]
fn manifest_lifecycle_optional() {
    let m = aenv_core::manifest::AenvManifest::from_toml(r#"name = "ns""#).unwrap();
    assert!(m.lifecycle.is_empty());
}

#[test]
fn manifest_lifecycle_partial() {
    let toml = r#"
name = "ns"

[lifecycle]
on_activate = "install.sh"
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.lifecycle.on_activate.as_deref(), Some("install.sh"));
    assert!(m.lifecycle.on_deactivate.is_none());
}

#[test]
fn manifest_lifecycle_rejects_absolute_path() {
    let toml = r#"
name = "ns"
[lifecycle]
on_activate = "/usr/bin/install"
"#;
    let err = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap_err();
    assert!(
        matches!(err, aenv_core::AenvError::ManifestInvalid(_)),
        "expected ManifestInvalid, got {err:?}"
    );
}

#[test]
fn manifest_lifecycle_rejects_parent_segment() {
    let toml = r#"
name = "ns"
[lifecycle]
on_activate = "../escape.sh"
"#;
    let err = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap_err();
    assert!(matches!(err, aenv_core::AenvError::ManifestInvalid(_)));
}

#[test]
fn manifest_lifecycle_rejects_tilde_prefix() {
    let toml = r#"
name = "ns"
[lifecycle]
on_activate = "~/install.sh"
"#;
    let err = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap_err();
    assert!(matches!(err, aenv_core::AenvError::ManifestInvalid(_)));
}

#[test]
fn manifest_lifecycle_roundtrips_through_to_toml() {
    let original = aenv_core::manifest::AenvManifest {
        name: "ns".into(),
        extends: vec![],
        adapters: std::collections::BTreeMap::new(),
        parameters: std::collections::BTreeMap::new(),
        policies: std::collections::BTreeMap::new(),
        skills: vec![],
        lifecycle: aenv_core::manifest::LifecycleHooks {
            on_activate: Some("install.sh".into()),
            on_deactivate: None,
        },
    };
    let toml = original.to_toml();
    let parsed = aenv_core::manifest::AenvManifest::from_toml(&toml).unwrap();
    assert_eq!(parsed.lifecycle, original.lifecycle);
}
