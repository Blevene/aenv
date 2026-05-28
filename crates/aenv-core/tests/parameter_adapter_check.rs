use aenv_core::adapter::{Adapter, AdapterParameterDecl, AdapterParameterType, AdapterRegistry};
use aenv_core::identity::NamespaceId;
use aenv_core::parameters::{check_against_adapters, ParameterValue, ResolvedParameter};
use std::collections::BTreeMap;

fn ns(name: &str) -> NamespaceId {
    NamespaceId::new(name).unwrap()
}

fn registry_with(adapters: Vec<Adapter>) -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    for a in adapters {
        r.insert(a);
    }
    r
}

fn ad(name: &str, params: Vec<(&str, AdapterParameterType)>) -> Adapter {
    Adapter {
        name: name.into(),
        files: vec![],
        merge_strategies: BTreeMap::new(),
        roles: BTreeMap::new(),
        default_merge: BTreeMap::new(),
        parameters: params
            .into_iter()
            .map(|(n, t)| AdapterParameterDecl {
                name: n.into(),
                r#type: t,
                projects_to: None,
            })
            .collect(),
        skills_dir: None,
        soft_limits: BTreeMap::new(),
        user_files: vec![],
        user_roles: BTreeMap::new(),
        user_default_merge: BTreeMap::new(),
        user_merge_strategies: BTreeMap::new(),
        user_soft_limits: BTreeMap::new(),
        user_skills_dir: None,
        materialize: None,
    }
}

fn rp(value: ParameterValue, source: &str) -> ResolvedParameter {
    ResolvedParameter {
        value,
        source: ns(source),
    }
}

#[test]
fn passes_when_types_match() {
    let registry = registry_with(vec![ad(
        "claude-code",
        vec![("default_model", AdapterParameterType::String)],
    )]);
    let mut resolved: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    resolved.insert(
        "default_model".into(),
        rp(ParameterValue::String("opus".into()), "leaf"),
    );

    check_against_adapters(&resolved, &registry).unwrap();
}

#[test]
fn fails_when_manifest_type_disagrees_with_adapter() {
    let registry = registry_with(vec![ad(
        "claude-code",
        vec![("default_model", AdapterParameterType::String)],
    )]);
    let mut resolved: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    resolved.insert(
        "default_model".into(),
        rp(ParameterValue::Integer(42), "leaf"),
    );

    let err = check_against_adapters(&resolved, &registry).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("default_model"), "msg = {msg}");
    assert!(msg.contains("string"), "msg = {msg}");
    assert!(msg.contains("integer"), "msg = {msg}");
}

#[test]
fn allows_parameters_not_declared_by_any_adapter() {
    // `forbid_tools` is consumed by downstream tooling; no adapter declares it.
    let registry = registry_with(vec![ad(
        "claude-code",
        vec![("default_model", AdapterParameterType::String)],
    )]);
    let mut resolved: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    resolved.insert(
        "forbid_tools".into(),
        rp(
            ParameterValue::ListString(vec!["edit".into(), "write".into()]),
            "leaf",
        ),
    );

    check_against_adapters(&resolved, &registry).unwrap();
}

#[test]
fn rejects_conflicting_adapter_declarations() {
    let registry = registry_with(vec![
        ad("a", vec![("x", AdapterParameterType::String)]),
        ad("b", vec![("x", AdapterParameterType::Integer)]),
    ]);
    let mut resolved: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    resolved.insert("x".into(), rp(ParameterValue::String("v".into()), "leaf"));

    let err = check_against_adapters(&resolved, &registry).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("'x'"), "msg = {msg}");
    assert!(msg.contains('a') && msg.contains('b'), "msg = {msg}");
}
