use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};

#[test]
fn namespace_id_roundtrips() {
    let id = NamespaceId::new("detailed-execution").unwrap();
    assert_eq!(id.as_str(), "detailed-execution");
    assert_eq!(format!("{id}"), "detailed-execution");
}

#[test]
fn short_name_roundtrips() {
    let sn = ShortName::new("write-tests").unwrap();
    assert_eq!(sn.as_str(), "write-tests");
}

#[test]
fn qualified_name_display_uses_double_colon() {
    let qn = QualifiedName::new(
        NamespaceId::new("detailed-execution").unwrap(),
        ShortName::new("write-tests").unwrap(),
    );
    assert_eq!(format!("{qn}"), "detailed-execution::write-tests");
}

#[test]
fn qualified_name_parses_from_str() {
    let qn: QualifiedName = "base::CLAUDE.md".parse().unwrap();
    assert_eq!(qn.namespace().as_str(), "base");
    assert_eq!(qn.short().as_str(), "CLAUDE.md");
}

#[test]
fn parse_rejects_missing_separator() {
    assert!("just-a-name".parse::<QualifiedName>().is_err());
}

#[test]
fn parse_rejects_empty_namespace() {
    assert!("::foo".parse::<QualifiedName>().is_err());
}

#[test]
fn parse_rejects_empty_short_name() {
    assert!("foo::".parse::<QualifiedName>().is_err());
}

#[test]
fn parse_rejects_double_separator_in_namespace() {
    // "a::b::c" is ambiguous; we reject rather than try to guess.
    assert!("a::b::c".parse::<QualifiedName>().is_err());
}

#[test]
fn namespace_id_rejects_empty() {
    assert!(NamespaceId::new("").is_err());
}

#[test]
fn namespace_id_rejects_colon_chars() {
    assert!(NamespaceId::new("foo::bar").is_err());
    assert!(NamespaceId::new("foo:bar").is_err());
}

#[test]
fn namespace_id_rejects_reserved_merged_synthetic() {
    let err = NamespaceId::new("(merged)").unwrap_err();
    assert!(err.to_string().contains("reserved"));
    assert!(err.to_string().contains("merged"));
}

#[test]
fn short_name_allows_path_separators() {
    // Short names can be paths (e.g. ".claude/skills/write-tests/SKILL.md").
    let sn = ShortName::new(".claude/skills/write-tests/SKILL.md").unwrap();
    assert_eq!(sn.as_str(), ".claude/skills/write-tests/SKILL.md");
}

#[test]
fn short_name_rejects_double_colon() {
    // The separator must never appear in a ShortName, even though paths are allowed.
    assert!(ShortName::new("foo::bar").is_err());
}

#[test]
fn qualified_name_is_hash_and_eq() {
    use std::collections::HashSet;
    let a = QualifiedName::new(
        NamespaceId::new("base").unwrap(),
        ShortName::new("write-tests").unwrap(),
    );
    let b = a.clone();
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}
