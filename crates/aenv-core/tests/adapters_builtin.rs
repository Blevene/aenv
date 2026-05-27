//! Tests for built-in adapters.

use aenv_core::adapter::Adapter;
use aenv_core::adapters_builtin::ALL;
use aenv_core::fs::{Filesystem, MockFilesystem};
use std::path::Path;

#[test]
fn all_eight_adapters_parse_cleanly() {
    assert_eq!(ALL.len(), 8);
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
    assert_eq!(
        parsed["codex"].roles.get("AGENTS.md").map(String::as_str),
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
fn ensure_written_creates_all_eight_files() {
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
fn claude_code_declares_user_files() {
    let toml = include_str!("../src/adapters_builtin/claude_code.toml");
    let a = aenv_core::adapter::Adapter::from_toml(toml).unwrap();
    assert!(a.user_files.contains(&"~/.claude/CLAUDE.md".to_string()));
    assert!(a.user_files.contains(&"~/.claude/agents/".to_string()));
    assert!(a
        .user_files
        .contains(&"~/.claude/settings.json".to_string()));
    assert_eq!(a.user_skills_dir.as_deref(), Some("~/.claude/skills"));
    assert_eq!(
        a.user_roles.get("~/.claude/CLAUDE.md").map(String::as_str),
        Some("instructions")
    );
    assert_eq!(a.user_soft_limits.get("instructions").copied(), Some(5000));
    assert_eq!(
        a.user_default_merge
            .get("~/.claude/settings.json")
            .map(String::as_str),
        Some("deep")
    );
}

#[test]
fn codex_declares_user_files() {
    let toml = include_str!("../src/adapters_builtin/codex.toml");
    let a = aenv_core::adapter::Adapter::from_toml(toml).unwrap();
    assert!(a.user_files.contains(&"~/.codex/AGENTS.md".to_string()));
    assert!(a.user_files.contains(&"~/.codex/config.toml".to_string()));
    assert_eq!(
        a.user_roles.get("~/.codex/AGENTS.md").map(String::as_str),
        Some("instructions")
    );
    assert_eq!(
        a.user_default_merge
            .get("~/.codex/config.toml")
            .map(String::as_str),
        Some("deep")
    );
}

#[test]
fn cursor_declares_user_files() {
    let toml = include_str!("../src/adapters_builtin/cursor.toml");
    let a = aenv_core::adapter::Adapter::from_toml(toml).unwrap();
    assert!(a.user_files.iter().any(|s| s.starts_with("~/.cursor/")));
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
