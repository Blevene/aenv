use aenv_core::adapter::{Adapter, AdapterParameterType};

#[test]
fn parses_no_parameters_when_absent() {
    let toml = r#"
name = "minimal"
files = ["a"]
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert!(a.parameters.is_empty());
}

#[test]
fn parses_all_four_types() {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md"]

[[parameters]]
name = "default_model"
type = "string"

[[parameters]]
name = "instructions_budget"
type = "integer"

[[parameters]]
name = "auto_invoke_subagents"
type = "list-of-string"

[[parameters]]
name = "verbose"
type = "boolean"
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert_eq!(a.parameters.len(), 4);
    assert_eq!(a.parameters[0].name, "default_model");
    assert_eq!(a.parameters[0].r#type, AdapterParameterType::String);
    assert_eq!(a.parameters[1].r#type, AdapterParameterType::Integer);
    assert_eq!(a.parameters[2].r#type, AdapterParameterType::ListString);
    assert_eq!(a.parameters[3].r#type, AdapterParameterType::Boolean);
}

#[test]
fn rejects_unknown_type() {
    let toml = r#"
name = "x"
[[parameters]]
name = "y"
type = "float"
"#;
    let err = Adapter::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("float"));
}

#[test]
fn optional_projection_target_is_captured() {
    let toml = r#"
name = "claude-code"
[[parameters]]
name = "auto_invoke_subagents"
type = "list-of-string"
projects_to = ".claude/settings.json"
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert_eq!(a.parameters.len(), 1);
    assert_eq!(
        a.parameters[0].projects_to.as_deref(),
        Some(".claude/settings.json")
    );
}

#[test]
fn rejects_duplicate_name_within_adapter() {
    let toml = r#"
name = "x"
[[parameters]]
name = "dup"
type = "string"

[[parameters]]
name = "dup"
type = "integer"
"#;
    let err = Adapter::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("dup"));
}
