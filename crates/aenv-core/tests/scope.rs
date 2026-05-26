//! Tests for the `Scope` enum.

use aenv_core::scope::Scope;

#[test]
fn scope_default_is_project() {
    assert_eq!(Scope::default(), Scope::Project);
}

#[test]
fn scope_serializes_as_lowercase() {
    let s = serde_json::to_string(&Scope::User).unwrap();
    assert_eq!(s, "\"user\"");
    let s = serde_json::to_string(&Scope::Project).unwrap();
    assert_eq!(s, "\"project\"");
}

#[test]
fn scope_deserializes_lowercase() {
    let s: Scope = serde_json::from_str("\"user\"").unwrap();
    assert_eq!(s, Scope::User);
    let s: Scope = serde_json::from_str("\"project\"").unwrap();
    assert_eq!(s, Scope::Project);
}

#[test]
fn scope_unknown_value_rejected() {
    let r: Result<Scope, _> = serde_json::from_str("\"system\"");
    assert!(r.is_err());
}

#[test]
fn scope_as_str_is_stable() {
    assert_eq!(Scope::Project.as_str(), "project");
    assert_eq!(Scope::User.as_str(), "user");
}
