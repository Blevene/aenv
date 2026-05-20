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
fn adapter_entry_fields_are_publicly_constructible() {
    // Compile-time check: AdapterEntry's fields stay pub. Downstream
    // consumers (Phase 2's composition layer) build these directly.
    let entry = AdapterEntry {
        files: vec!["CLAUDE.md".to_string()],
    };
    assert_eq!(entry.files.len(), 1);
}
