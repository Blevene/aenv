use aenv_core::identity::NamespaceId;
use aenv_core::policies::{resolve_policies, PolicyDecl, PolicyValue, ResolvedPolicy};
use std::collections::BTreeMap;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn pd_int(i: i64, enforce: bool) -> PolicyDecl {
    PolicyDecl {
        value: PolicyValue::Integer(i),
        enforce,
    }
}

fn pd_bool(b: bool, enforce: bool) -> PolicyDecl {
    PolicyDecl {
        value: PolicyValue::Boolean(b),
        enforce,
    }
}

fn pd_list(xs: &[&str], enforce: bool) -> PolicyDecl {
    PolicyDecl {
        value: PolicyValue::ListString(xs.iter().map(|s| (*s).into()).collect()),
        enforce,
    }
}

#[test]
fn single_namespace_passes_through() {
    let chain = vec![ns("base")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(5000, false))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    let p = resolved.get("k").unwrap();
    assert_eq!(p.value, PolicyValue::Integer(5000));
    assert!(!p.enforce);
    assert_eq!(p.source, ns("base"));
}

#[test]
fn child_advisory_override_wins() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(5000, false))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_int(3000, false))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    let p = resolved.get("k").unwrap();
    assert_eq!(p.value, PolicyValue::Integer(3000));
    assert_eq!(p.source, ns("leaf"));
}

#[test]
fn child_can_upgrade_advisory_to_enforce() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(5000, false))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_int(3000, true))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    let p = resolved.get("k").unwrap();
    assert!(p.enforce);
    assert_eq!(p.source, ns("leaf"));
}

#[test]
fn child_cannot_downgrade_enforce_to_advisory() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(3000, true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_int(3000, false))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    assert!(err.to_string().contains("'k'"));
    assert!(err.to_string().contains("enforce"));
}

#[test]
fn child_cannot_raise_enforced_int_limit() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("instructions_max_chars".into(), pd_int(3000, true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("instructions_max_chars".into(), pd_int(5000, true))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("instructions_max_chars"));
    assert!(msg.contains("weaken") || msg.contains("raise"));
}

#[test]
fn child_can_lower_enforced_int_limit() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(5000, true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_int(3000, true))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    assert_eq!(resolved.get("k").unwrap().value, PolicyValue::Integer(3000));
}

#[test]
fn child_cannot_flip_enforced_true_to_false() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_bool(true, true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_bool(false, true))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    assert!(err.to_string().contains("'k'"));
}

#[test]
fn child_cannot_shrink_enforced_deny_list() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([(
            "forbid_paths".into(),
            pd_list(&["secrets/**", ".env*"], true),
        )]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("forbid_paths".into(), pd_list(&[".env*"], true))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    assert!(err.to_string().contains("forbid_paths"));
    assert!(err.to_string().contains("secrets"));
}

#[test]
fn child_can_extend_enforced_deny_list() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("forbid_paths".into(), pd_list(&["a"], true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("forbid_paths".into(), pd_list(&["a", "b"], true))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    let p = resolved.get("forbid_paths").unwrap();
    let xs = match &p.value {
        PolicyValue::ListString(xs) => xs.clone(),
        _ => panic!(),
    };
    assert!(xs.contains(&"a".to_string()));
    assert!(xs.contains(&"b".to_string()));
}

#[test]
fn type_mismatch_across_chain_errors() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(ns("base"), BTreeMap::from([("k".into(), pd_int(1, false))]));
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_bool(false, false))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    assert!(err.to_string().contains("'k'"));
    assert!(err.to_string().contains("integer"));
    assert!(err.to_string().contains("boolean"));
}

#[test]
fn fields_accessible_via_struct() {
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(42),
        enforce: true,
        source: ns("base"),
    };
    assert_eq!(rp.value, PolicyValue::Integer(42));
    assert!(rp.enforce);
    assert_eq!(rp.source.as_str(), "base");
}
