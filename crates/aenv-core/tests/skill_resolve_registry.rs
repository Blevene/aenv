use aenv_core::skills::registry::resolve_registry;

#[test]
fn registry_returns_not_yet_implemented() {
    let err = resolve_registry("cite-evidence", None).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("registry") && msg.contains("not yet implemented"));
}
