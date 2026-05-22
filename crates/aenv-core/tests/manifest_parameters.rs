use aenv_core::manifest::AenvManifest;
use aenv_core::parameters::ParameterValue;

#[test]
fn parses_all_four_types() {
    let toml = r#"
name = "detailed-execution"

[parameters]
default_model = "claude-opus-4.7"
instructions_budget = 3000
auto_invoke_subagents = true
forbid_tools = ["edit", "write"]
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(
        m.parameters.get("default_model"),
        Some(&ParameterValue::String("claude-opus-4.7".into()))
    );
    assert_eq!(
        m.parameters.get("instructions_budget"),
        Some(&ParameterValue::Integer(3000))
    );
    assert_eq!(
        m.parameters.get("auto_invoke_subagents"),
        Some(&ParameterValue::Boolean(true))
    );
    assert_eq!(
        m.parameters.get("forbid_tools"),
        Some(&ParameterValue::ListString(vec![
            "edit".into(),
            "write".into()
        ]))
    );
}

#[test]
fn missing_block_is_empty_map() {
    let toml = r#"name = "base""#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert!(m.parameters.is_empty());
}

#[test]
fn rejects_float_value() {
    let toml = r#"
name = "x"
[parameters]
bad = 1.5
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("float"));
    assert!(err.to_string().contains("bad"));
}

#[test]
fn rejects_mixed_array() {
    let toml = r#"
name = "x"
[parameters]
bad = ["ok", 7]
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("list") || err.to_string().contains("bad"));
}

#[test]
fn roundtrip_preserves_parameters() {
    let toml = r#"
name = "x"

[parameters]
default_model = "claude-opus-4.7"
budget = 3000
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let rendered = m.to_toml();
    let m2 = AenvManifest::from_toml(&rendered).unwrap();
    assert_eq!(m, m2);
}
