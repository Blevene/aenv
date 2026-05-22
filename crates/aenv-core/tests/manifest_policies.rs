use aenv_core::manifest::AenvManifest;
use aenv_core::policies::{PolicyDecl, PolicyValue};

#[test]
fn parses_shorthand_advisory() {
    let toml = r#"
name = "base"

[policies]
instructions_max_chars = 5000
skill_requires_description = true
forbid_paths = [".env*", "secrets/**"]
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(
        m.policies.get("instructions_max_chars"),
        Some(&PolicyDecl {
            value: PolicyValue::Integer(5000),
            enforce: false,
        })
    );
    assert_eq!(
        m.policies.get("skill_requires_description"),
        Some(&PolicyDecl {
            value: PolicyValue::Boolean(true),
            enforce: false,
        })
    );
    assert_eq!(
        m.policies.get("forbid_paths"),
        Some(&PolicyDecl {
            value: PolicyValue::ListString(vec![".env*".into(), "secrets/**".into()]),
            enforce: false,
        })
    );
}

#[test]
fn parses_table_form_enforce() {
    let toml = r#"
name = "leaf"

[policies]
instructions_max_chars = { value = 3000, enforce = true }
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(
        m.policies.get("instructions_max_chars"),
        Some(&PolicyDecl {
            value: PolicyValue::Integer(3000),
            enforce: true,
        })
    );
}

#[test]
fn parses_table_form_explicit_advisory() {
    let toml = r#"
name = "x"

[policies]
forbid_paths = { value = ["a"], enforce = false }
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(
        m.policies.get("forbid_paths").unwrap().enforce,
        false
    );
}

#[test]
fn missing_block_is_empty_map() {
    let toml = r#"name = "x""#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert!(m.policies.is_empty());
}

#[test]
fn rejects_string_value() {
    let toml = r#"
name = "x"

[policies]
weird = "this is not a valid policy value"
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("weird"));
}

#[test]
fn rejects_table_without_value_field() {
    let toml = r#"
name = "x"

[policies]
bad = { enforce = true }
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("bad"));
}

#[test]
fn rejects_mixed_list_value() {
    let toml = r#"
name = "x"

[policies]
forbid_paths = ["ok", 5]
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(
        err.to_string().contains("forbid_paths") || err.to_string().contains("list"),
        "msg = {err}"
    );
}
