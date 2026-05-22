//! Tests for adapter `soft_limits` field.

use aenv_core::adapter::Adapter;

#[test]
fn parses_soft_limits() {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md"]

[soft_limits]
instructions = 5000
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert_eq!(a.soft_limits.get("instructions"), Some(&5000));
}

#[test]
fn missing_block_is_empty_map() {
    let toml = r#"
name = "x"
files = ["a"]
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert!(a.soft_limits.is_empty());
}

#[test]
fn builtins_declare_expected_limits() {
    use aenv_core::adapters_builtin::ALL;
    let mut found_claude = false;
    let mut found_windsurf = false;
    for (name, toml) in ALL {
        let adapter = Adapter::from_toml(toml).unwrap();
        match *name {
            "claude-code" => {
                assert_eq!(adapter.soft_limits.get("instructions"), Some(&5000));
                found_claude = true;
            }
            "windsurf" => {
                assert_eq!(adapter.soft_limits.get("instructions"), Some(&6000));
                found_windsurf = true;
            }
            "cursor" | "cline" | "continue" | "aider" => {
                assert_eq!(
                    adapter.soft_limits.get("instructions"),
                    Some(&5000),
                    "{name} should declare instructions=5000"
                );
            }
            "mcp" => {
                assert!(
                    !adapter.soft_limits.contains_key("instructions"),
                    "mcp has no instructions role"
                );
            }
            other => panic!("unexpected built-in adapter '{other}'"),
        }
    }
    assert!(found_claude);
    assert!(found_windsurf);
}
