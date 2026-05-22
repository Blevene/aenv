use aenv_core::skills::source::SourceKind;

#[test]
fn parses_local_absolute() {
    let s = SourceKind::parse("/home/user/skills/foo").unwrap();
    match s {
        SourceKind::Local(p) => assert_eq!(p.to_string_lossy(), "/home/user/skills/foo"),
        _ => panic!("expected Local, got {s:?}"),
    }
}

#[test]
fn parses_local_tilde_unexpanded() {
    let s = SourceKind::parse("~/team-skills/foo").unwrap();
    match s {
        SourceKind::Local(p) => assert_eq!(p.to_string_lossy(), "~/team-skills/foo"),
        _ => panic!("expected Local, got {s:?}"),
    }
}

#[test]
fn parses_git_url_with_fragment_ref() {
    let s =
        SourceKind::parse("git+https://github.com/acme/aenv-skills.git#match-conventions").unwrap();
    match s {
        SourceKind::Git { url, ref_spec } => {
            assert_eq!(url, "https://github.com/acme/aenv-skills.git");
            assert_eq!(ref_spec.as_deref(), Some("match-conventions"));
        }
        _ => panic!("expected Git, got {s:?}"),
    }
}

#[test]
fn parses_git_url_without_fragment() {
    let s = SourceKind::parse("git+https://github.com/acme/aenv-skills.git").unwrap();
    match s {
        SourceKind::Git { url, ref_spec } => {
            assert_eq!(url, "https://github.com/acme/aenv-skills.git");
            assert!(ref_spec.is_none());
        }
        _ => panic!("expected Git, got {s:?}"),
    }
}

#[test]
fn parses_registry_source() {
    let s = SourceKind::parse("registry:cite-evidence").unwrap();
    match s {
        SourceKind::Registry(name) => assert_eq!(name, "cite-evidence"),
        _ => panic!("expected Registry, got {s:?}"),
    }
}

#[test]
fn rejects_unknown_prefix() {
    let err = SourceKind::parse("https://example.com/skill.zip").unwrap_err();
    assert!(err.to_string().contains("source"));
}

#[test]
fn rejects_empty() {
    let err = SourceKind::parse("").unwrap_err();
    assert!(err.to_string().contains("empty") || err.to_string().contains("source"));
}

#[test]
fn rejects_relative_local_path() {
    let err = SourceKind::parse("./my-skill").unwrap_err();
    assert!(err.to_string().contains("relative") || err.to_string().contains("absolute"));
}
