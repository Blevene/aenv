//! Locks the exit-code contract from PRD R-82.
//!
//! These codes are public; changing them is a major-version break.

use aenv_core::AenvError;

#[test]
fn generic_io_maps_to_exit_one() {
    let err = AenvError::Io(std::io::Error::other("boom"));
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn namespace_not_found_is_ten() {
    let err = AenvError::NamespaceNotFound("missing".to_string());
    assert_eq!(err.exit_code(), 10);
}

#[test]
fn adapter_missing_is_eleven() {
    let err = AenvError::AdapterMissing("nope".to_string());
    assert_eq!(err.exit_code(), 11);
}

#[test]
fn manifest_invalid_is_twelve() {
    let err = AenvError::ManifestInvalid("bad toml".to_string());
    assert_eq!(err.exit_code(), 12);
}

#[test]
fn activation_conflict_is_thirteen() {
    let err = AenvError::ActivationConflict("file exists".to_string());
    assert_eq!(err.exit_code(), 13);
}

#[test]
fn remote_unreachable_is_fourteen() {
    let err = AenvError::RemoteUnreachable("git fetch failed".to_string());
    assert_eq!(err.exit_code(), 14);
}

#[test]
fn extends_cycle_is_fifteen() {
    let err = AenvError::ExtendsCycle("a -> b -> a".to_string());
    assert_eq!(err.exit_code(), 15);
}

#[test]
fn parameter_undefined_is_sixteen() {
    let err = AenvError::ParameterUndefined("foo.bar".to_string());
    assert_eq!(err.exit_code(), 16);
}

#[test]
fn policy_violation_is_seventeen() {
    let err = AenvError::PolicyViolation("oversize".to_string());
    assert_eq!(err.exit_code(), 17);
}

#[test]
fn project_not_pinned_is_twenty() {
    let err = AenvError::ProjectNotPinned;
    assert_eq!(err.exit_code(), 20);
}

#[test]
fn display_includes_namespace_in_not_found_message() {
    // PRD-driven: error messages should use the "namespace" vocabulary in
    // user-visible output (engineering doc §3 rationale).
    let err = AenvError::NamespaceNotFound("foo".to_string());
    let msg = format!("{}", err);
    assert!(
        msg.contains("namespace"),
        "expected 'namespace' in {:?}",
        msg
    );
    assert!(msg.contains("foo"), "expected 'foo' in {:?}", msg);
}
