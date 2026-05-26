use aenv_core::manifest::AenvManifest;
use aenv_core::skills::SkillMode;

#[test]
fn parses_authored_skill() {
    let toml = r#"
name = "experiments"

[[skills]]
name = "compare-approaches"
mode = "authored"
adapter = "claude-code"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills.len(), 1);
    assert_eq!(m.skills[0].name, "compare-approaches");
    assert!(matches!(m.skills[0].mode, SkillMode::Authored));
    assert_eq!(m.skills[0].adapter.as_deref(), Some("claude-code"));
}

#[test]
fn parses_imported_skill_pinned() {
    let toml = r#"
name = "detailed-execution"

[[skills]]
name = "match-conventions"
mode = "imported"
adapter = "claude-code"
source = "git+https://github.com/acme/aenv-skills.git#match-conventions"
ref = "v1.2.0"
required = true
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills.len(), 1);
    assert!(matches!(m.skills[0].mode, SkillMode::Imported));
    assert!(m.skills[0].required);
    assert_eq!(m.skills[0].ref_.as_deref(), Some("v1.2.0"));
}

#[test]
fn parses_multiple_skills() {
    let toml = r#"
name = "x"

[[skills]]
name = "a"
mode = "authored"

[[skills]]
name = "b"
mode = "imported"
source = "/local/path/b"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills.len(), 2);
    assert_eq!(m.skills[0].name, "a");
    assert_eq!(m.skills[1].name, "b");
}

#[test]
fn missing_block_is_empty_vec() {
    let toml = r#"name = "x""#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert!(m.skills.is_empty());
}

#[test]
fn rejects_imported_without_source() {
    let toml = r#"
name = "x"

[[skills]]
name = "needs-source"
mode = "imported"
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("needs-source"));
    assert!(err.to_string().contains("source"));
}

#[test]
fn rejects_authored_with_source() {
    let toml = r#"
name = "x"

[[skills]]
name = "stray"
mode = "authored"
source = "/somewhere"
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("stray"));
    assert!(err.to_string().contains("source"));
}

#[test]
fn rejects_duplicate_skill_names() {
    let toml = r#"
name = "x"

[[skills]]
name = "dup"
mode = "authored"

[[skills]]
name = "dup"
mode = "authored"
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("dup"));
}

#[test]
fn roundtrip_preserves_skills() {
    let toml = r#"
name = "x"

[[skills]]
name = "a"
mode = "imported"
source = "/p"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let rendered = m.to_toml();
    let m2 = AenvManifest::from_toml(&rendered).unwrap();
    assert_eq!(m, m2);
}

#[test]
fn skill_scope_defaults_to_project() {
    let toml = r#"
name = "ns"
[[skills]]
name = "code-reviewer"
adapter = "claude-code"
mode = "authored"
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills[0].scope, aenv_core::scope::Scope::Project);
}

#[test]
fn skill_scope_user_round_trips() {
    let toml = r#"
name = "ns"
[[skills]]
name = "personal-helper"
adapter = "claude-code"
mode = "authored"
scope = "user"
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills[0].scope, aenv_core::scope::Scope::User);
}
