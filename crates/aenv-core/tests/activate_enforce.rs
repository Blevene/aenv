use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::AenvError;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::PathBuf;

fn make_layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/h"))
}

fn install_claude_adapter(fs: &MockFilesystem, layout: &RegistryLayout) -> AdapterRegistry {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md"]

[roles]
"CLAUDE.md" = "instructions"
"#;
    fs.write(
        &layout.adapters_dir().join("claude-code.toml"),
        toml.as_bytes(),
    )
    .unwrap();
    AdapterRegistry::load_from_dir(fs, &layout.adapters_dir()).unwrap()
}

fn write_manifest(fs: &MockFilesystem, layout: &RegistryLayout, name: &str, body: &str) {
    fs.write(&layout.manifest_path(name), body.as_bytes()).unwrap();
}

#[test]
fn activation_refused_when_enforce_violation() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = install_claude_adapter(&fs, &layout);

    let manifest = r#"
name = "tight"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = { value = 100, enforce = true }
"#;
    write_manifest(&fs, &layout, "tight", manifest);
    let body = "x".repeat(500);
    fs.write(
        &layout.namespace_dir("tight").join("CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let err = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("tight").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, AenvError::PolicyViolation(_)));
    assert_eq!(err.exit_code(), 17);
    // No state file should have been written.
    assert!(!fs.exists(&project.join(".aenv-state/state.json")).unwrap());
    // No symlink should exist in the project.
    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn advisory_violation_does_not_block_activation() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = install_claude_adapter(&fs, &layout);

    let manifest = r#"
name = "loose"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = 100
"#;
    write_manifest(&fs, &layout, "loose", manifest);
    let body = "x".repeat(500);
    fs.write(
        &layout.namespace_dir("loose").join("CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("loose").unwrap(),
    )
    .unwrap();
    assert!(fs.exists(&project.join(".aenv-state/state.json")).unwrap());
}

#[test]
fn enforce_violation_message_names_policy_and_namespace() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = install_claude_adapter(&fs, &layout);
    let manifest = r#"
name = "tight"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = { value = 100, enforce = true }
"#;
    write_manifest(&fs, &layout, "tight", manifest);
    fs.write(
        &layout.namespace_dir("tight").join("CLAUDE.md"),
        "x".repeat(500).as_bytes(),
    )
    .unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let err = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("tight").unwrap(),
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("instructions_max_chars"), "msg = {msg}");
    assert!(msg.contains("100"), "msg = {msg}");
}
