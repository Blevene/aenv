use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::skills::{apply_required_rule, resolve_imported_skill, SkillDecl, SkillMode};
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv-home"))
}

#[test]
fn resolves_when_local_source_exists() {
    let fs = MockFilesystem::new();
    fs.write(
        &PathBuf::from("/local/skill/SKILL.md"),
        b"---\nname: x\ndescription: y\n---\n",
    )
    .unwrap();
    let decl = SkillDecl {
        name: "my-skill".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/local/skill".into()),
        ref_: None,
        required: false,
    };
    let result = resolve_imported_skill(&fs, &layout(), &decl).unwrap();
    assert_eq!(result.source_path, PathBuf::from("/local/skill"));
}

#[test]
fn required_unreachable_propagates_error() {
    let fs = MockFilesystem::new();
    let decl = SkillDecl {
        name: "missing".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/does/not/exist".into()),
        ref_: None,
        required: true,
    };
    let outcome = apply_required_rule(&fs, &layout(), &decl);
    let err = outcome.expect_err("required + missing should error");
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn unrequired_unreachable_returns_skipped_marker() {
    let fs = MockFilesystem::new();
    let decl = SkillDecl {
        name: "optional".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/does/not/exist".into()),
        ref_: None,
        required: false,
    };
    let outcome = apply_required_rule(&fs, &layout(), &decl).unwrap();
    assert!(outcome.is_none());
}

#[test]
fn authored_decls_panic_or_error() {
    let fs = MockFilesystem::new();
    let decl = SkillDecl {
        name: "x".into(),
        mode: SkillMode::Authored,
        adapter: None,
        source: None,
        ref_: None,
        required: false,
    };
    let err = apply_required_rule(&fs, &layout(), &decl).unwrap_err();
    assert!(err.to_string().contains("authored"));
}
