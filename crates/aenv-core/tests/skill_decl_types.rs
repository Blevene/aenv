use aenv_core::skills::{SkillDecl, SkillMode};

#[test]
fn authored_decl_shape() {
    let s = SkillDecl {
        name: "write-tests".into(),
        mode: SkillMode::Authored,
        adapter: Some("claude-code".into()),
        source: None,
        ref_: None,
        path: None,
        required: false,
    };
    assert_eq!(s.name, "write-tests");
    assert!(matches!(s.mode, SkillMode::Authored));
    assert_eq!(s.adapter.as_deref(), Some("claude-code"));
    assert!(s.source.is_none());
}

#[test]
fn imported_decl_shape() {
    let s = SkillDecl {
        name: "match-conventions".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("git+https://github.com/acme/aenv-skills.git#match-conventions".into()),
        ref_: Some("v1.2.0".into()),
        path: None,
        required: true,
    };
    assert!(matches!(s.mode, SkillMode::Imported));
    assert!(s.required);
    assert_eq!(s.ref_.as_deref(), Some("v1.2.0"));
}

#[test]
fn skill_mode_round_trips_via_serde() {
    let authored = SkillMode::Authored;
    let json = serde_json::to_string(&authored).unwrap();
    assert_eq!(json, "\"authored\"");
    let back: SkillMode = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, SkillMode::Authored));

    let imported = SkillMode::Imported;
    let json = serde_json::to_string(&imported).unwrap();
    assert_eq!(json, "\"imported\"");
    let back: SkillMode = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, SkillMode::Imported));
}

#[test]
fn skill_decl_round_trips_via_toml() {
    let s = SkillDecl {
        name: "x".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/local/path".into()),
        ref_: None,
        path: None,
        required: false,
    };
    let rendered = toml::to_string(&s).unwrap();
    let back: SkillDecl = toml::from_str(&rendered).unwrap();
    assert_eq!(s, back);
}
