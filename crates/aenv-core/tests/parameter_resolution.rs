use aenv_core::identity::NamespaceId;
use aenv_core::parameters::{resolve_parameters, ParameterValue, ResolvedParameter};
use std::collections::BTreeMap;

fn ns(name: &str) -> NamespaceId {
    NamespaceId::new(name).unwrap()
}

fn pv_string(s: &str) -> ParameterValue {
    ParameterValue::String(s.into())
}

#[test]
fn single_namespace_passes_through() {
    let chain = vec![ns("base")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("default_model".into(), pv_string("haiku"))]),
    );

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    let p = resolved.get("default_model").unwrap();
    assert_eq!(p.value, pv_string("haiku"));
    assert_eq!(p.source, ns("base"));
}

#[test]
fn child_overrides_parent() {
    let chain = vec![ns("base"), ns("detailed-execution")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("default_model".into(), pv_string("haiku"))]),
    );
    per_ns.insert(
        ns("detailed-execution"),
        BTreeMap::from([("default_model".into(), pv_string("opus"))]),
    );

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    let p = resolved.get("default_model").unwrap();
    assert_eq!(p.value, pv_string("opus"));
    assert_eq!(p.source, ns("detailed-execution"));
}

#[test]
fn parent_only_keys_pass_through() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("budget".into(), ParameterValue::Integer(5000))]),
    );
    per_ns.insert(ns("leaf"), BTreeMap::new());

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    let p = resolved.get("budget").unwrap();
    assert_eq!(p.value, ParameterValue::Integer(5000));
    assert_eq!(p.source, ns("base"));
}

#[test]
fn type_mismatch_across_chain_errors() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("budget".into(), ParameterValue::Integer(5000))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("budget".into(), pv_string("a lot"))]),
    );

    let err = resolve_parameters(&chain, &per_ns).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("budget"), "msg = {msg}");
    assert!(
        msg.contains("integer") && msg.contains("string"),
        "expected both type tags in msg, got: {msg}"
    );
}

#[test]
fn three_level_chain_last_wins() {
    let chain = vec![ns("root"), ns("mid"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("root"),
        BTreeMap::from([("model".into(), pv_string("haiku"))]),
    );
    per_ns.insert(
        ns("mid"),
        BTreeMap::from([("model".into(), pv_string("sonnet"))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("model".into(), pv_string("opus"))]),
    );

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    let p = resolved.get("model").unwrap();
    assert_eq!(p.value, pv_string("opus"));
    assert_eq!(p.source, ns("leaf"));
}

#[test]
fn unrelated_keys_dont_clash() {
    let chain = vec![ns("a"), ns("b")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("a"),
        BTreeMap::from([("x".into(), ParameterValue::Integer(1))]),
    );
    per_ns.insert(
        ns("b"),
        BTreeMap::from([("y".into(), ParameterValue::Boolean(true))]),
    );

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved.get("x").unwrap().source, ns("a"));
    assert_eq!(resolved.get("y").unwrap().source, ns("b"));
}

#[test]
fn fields_accessible_via_struct() {
    // Sanity: the `ResolvedParameter` API uses public `value` and `source`.
    let rp = ResolvedParameter {
        value: ParameterValue::Integer(42),
        source: ns("base"),
    };
    assert_eq!(rp.value, ParameterValue::Integer(42));
    assert_eq!(rp.source.as_str(), "base");
}
