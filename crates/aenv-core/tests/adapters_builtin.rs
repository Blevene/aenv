//! Tests for built-in adapters.

use aenv_core::adapter::Adapter;
use aenv_core::adapters_builtin::ALL;
use aenv_core::fs::{Filesystem, MockFilesystem};
use std::path::Path;

#[test]
fn all_seven_adapters_parse_cleanly() {
    assert_eq!(ALL.len(), 7);
    for (name, body) in ALL {
        let parsed: Adapter =
            toml::from_str(body).unwrap_or_else(|e| panic!("adapter {name} failed to parse: {e}"));
        assert_eq!(
            parsed.name, *name,
            "adapter file {name} declares name = {:?}",
            parsed.name
        );
        assert!(!parsed.files.is_empty(), "adapter {name} declares no files");
    }
}

#[test]
fn instructions_role_present_on_text_rules_adapters() {
    let parsed: std::collections::BTreeMap<&str, Adapter> = ALL
        .iter()
        .map(|(n, body)| (*n, toml::from_str(body).unwrap()))
        .collect();
    assert_eq!(
        parsed["claude-code"]
            .roles
            .get("CLAUDE.md")
            .map(String::as_str),
        Some("instructions")
    );
    assert_eq!(
        parsed["cursor"]
            .roles
            .get(".cursorrules")
            .map(String::as_str),
        Some("instructions")
    );
    assert_eq!(
        parsed["cline"].roles.get(".clinerules").map(String::as_str),
        Some("instructions")
    );
    assert_eq!(
        parsed["windsurf"]
            .roles
            .get(".windsurfrules")
            .map(String::as_str),
        Some("instructions")
    );
}

#[test]
fn deep_default_merge_on_structured_files() {
    let parsed: std::collections::BTreeMap<&str, Adapter> = ALL
        .iter()
        .map(|(n, body)| (*n, toml::from_str(body).unwrap()))
        .collect();
    assert_eq!(
        parsed["mcp"]
            .default_merge
            .get(".mcp.json")
            .map(String::as_str),
        Some("deep")
    );
    assert_eq!(
        parsed["aider"]
            .default_merge
            .get(".aider.conf.yml")
            .map(String::as_str),
        Some("deep")
    );
    assert_eq!(
        parsed["continue"]
            .default_merge
            .get(".continue/config.json")
            .map(String::as_str),
        Some("deep")
    );
}

#[test]
fn ensure_written_creates_all_seven_files() {
    let fs = MockFilesystem::new();
    let dir = Path::new("/aenv/adapters");
    aenv_core::adapters_builtin::ensure_written(&fs, dir).unwrap();
    for (name, _) in aenv_core::adapters_builtin::ALL {
        let path = dir.join(format!("{name}.toml"));
        assert!(
            fs.exists(&path).unwrap(),
            "expected {} to exist",
            path.display()
        );
    }
}

#[test]
fn ensure_written_leaves_existing_files_untouched() {
    let fs = MockFilesystem::new();
    let dir = Path::new("/aenv/adapters");
    fs.write(&dir.join("cursor.toml"), b"user-customized\n")
        .unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, dir).unwrap();
    let body = String::from_utf8(fs.read(&dir.join("cursor.toml")).unwrap()).unwrap();
    assert_eq!(body, "user-customized\n");
}
