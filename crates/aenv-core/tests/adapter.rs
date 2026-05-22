//! Tests for adapter TOML parsing and the built-in registry.

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::adapters_builtin;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::AenvError;
use std::path::PathBuf;

#[test]
fn parses_minimal_adapter() {
    let toml = r#"
        name = "claude-code"
        files = ["CLAUDE.md", ".claude/"]
    "#;
    let a = Adapter::from_toml(toml).unwrap();
    assert_eq!(a.name, "claude-code");
    assert_eq!(
        a.files,
        vec!["CLAUDE.md".to_string(), ".claude/".to_string()]
    );
}

#[test]
fn rejects_missing_name() {
    let toml = r#"files = ["CLAUDE.md"]"#;
    let err = Adapter::from_toml(toml).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn rejects_malformed_toml() {
    let toml = r#"name = ::: nope"#;
    let err = Adapter::from_toml(toml).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn registry_starts_empty() {
    let reg = AdapterRegistry::new();
    assert!(reg.get("anything").is_none());
    assert_eq!(reg.len(), 0);
}

#[test]
fn registry_insert_then_lookup() {
    let mut reg = AdapterRegistry::new();
    let a = Adapter {
        name: "claude-code".to_string(),
        files: vec!["CLAUDE.md".to_string()],
        merge_strategies: Default::default(),
        roles: Default::default(),
        default_merge: Default::default(),
        parameters: vec![],
    };
    reg.insert(a.clone());
    assert_eq!(reg.get("claude-code"), Some(&a));
    assert_eq!(reg.len(), 1);
}

#[test]
fn builtin_claude_code_parses() {
    // The embedded claude-code adapter must itself be valid TOML.
    let toml = adapters_builtin::CLAUDE_CODE_TOML;
    let a = Adapter::from_toml(toml).expect("embedded claude-code must parse");
    assert_eq!(a.name, "claude-code");
    assert!(a.files.iter().any(|f| f == "CLAUDE.md"));
}

#[test]
fn install_builtins_writes_claude_code_to_disk() {
    let fs = MockFilesystem::new();
    let adapters_dir = PathBuf::from("/aenv/adapters");
    adapters_builtin::install_builtins(&fs, &adapters_dir).unwrap();
    let written = fs.read(&adapters_dir.join("claude-code.toml")).unwrap();
    let parsed = Adapter::from_toml(&String::from_utf8(written).unwrap()).unwrap();
    assert_eq!(parsed.name, "claude-code");
}

#[test]
fn install_builtins_is_idempotent_for_unchanged_files() {
    let fs = MockFilesystem::new();
    let adapters_dir = PathBuf::from("/aenv/adapters");
    adapters_builtin::install_builtins(&fs, &adapters_dir).unwrap();
    adapters_builtin::install_builtins(&fs, &adapters_dir).unwrap();
    let parsed = Adapter::from_toml(
        &String::from_utf8(fs.read(&adapters_dir.join("claude-code.toml")).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(parsed.name, "claude-code");
}

#[test]
fn install_builtins_does_not_overwrite_user_modified_file() {
    let fs = MockFilesystem::new();
    let adapters_dir = PathBuf::from("/aenv/adapters");
    let path = adapters_dir.join("claude-code.toml");
    let user_content = b"name = \"claude-code\"\nfiles = [\"only-this.md\"]\n";
    fs.write(&path, user_content).unwrap();

    adapters_builtin::install_builtins(&fs, &adapters_dir).unwrap();

    assert_eq!(fs.read(&path).unwrap(), user_content);
}

#[test]
fn load_adapters_dir_reads_all_files() {
    let fs = MockFilesystem::new();
    let dir = PathBuf::from("/aenv/adapters");
    fs.write(
        &dir.join("claude-code.toml"),
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    fs.write(
        &dir.join("cursor.toml"),
        b"name = \"cursor\"\nfiles = [\".cursorrules\"]\n",
    )
    .unwrap();

    let reg = AdapterRegistry::load_from_dir(&fs, &dir).unwrap();
    assert_eq!(reg.len(), 2);
    assert!(reg.get("claude-code").is_some());
    assert!(reg.get("cursor").is_some());
}

#[test]
fn load_adapters_dir_skips_non_toml_files() {
    let fs = MockFilesystem::new();
    let dir = PathBuf::from("/aenv/adapters");
    fs.write(
        &dir.join("claude-code.toml"),
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    fs.write(&dir.join("README"), b"not a toml file\n").unwrap();

    let reg = AdapterRegistry::load_from_dir(&fs, &dir).unwrap();
    assert_eq!(reg.len(), 1);
}

#[test]
fn adapter_parses_roles_and_default_merge() {
    let toml = r#"
name = "mcp"
files = [".mcp.json"]
[default_merge]
".mcp.json" = "deep"
"#;
    let a: aenv_core::adapter::Adapter = toml::from_str(toml).unwrap();
    assert_eq!(a.default_merge.get(".mcp.json").unwrap(), "deep");
}

#[test]
fn adapter_parses_role_declaration() {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"#;
    let a: aenv_core::adapter::Adapter = toml::from_str(toml).unwrap();
    assert_eq!(a.roles.get("CLAUDE.md").unwrap(), "instructions");
}
