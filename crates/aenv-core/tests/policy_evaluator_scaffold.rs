use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::policies::builtin::{dispatch, OutcomeStatus, PolicyContext, PolicyOutcome};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};

#[test]
fn unknown_key_returns_warn_skip() {
    let ctx = PolicyContext::dummy();
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: NamespaceId::new("base").unwrap(),
    };
    let out = dispatch("does_not_exist", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::WarnSkip { .. }));
    assert_eq!(out[0].key, "does_not_exist");
}

#[test]
fn outcome_struct_shape() {
    let o = PolicyOutcome {
        key: "k".into(),
        target: Some(QualifiedName::new(
            NamespaceId::new("base").unwrap(),
            ShortName::new("CLAUDE.md").unwrap(),
        )),
        status: OutcomeStatus::Pass,
    };
    assert_eq!(o.key, "k");
    assert!(o.target.is_some());
}

#[test]
fn pass_constructor_helper() {
    let o = PolicyOutcome::pass("k", None);
    assert!(matches!(o.status, OutcomeStatus::Pass));
}

#[test]
fn fail_constructor_helper() {
    let o = PolicyOutcome::fail("k", None, "reason");
    if let OutcomeStatus::Fail { msg } = &o.status {
        assert_eq!(msg, "reason");
    } else {
        panic!("expected Fail");
    }
}

#[test]
fn warn_constructor_helper() {
    let o = PolicyOutcome::warn("k", None, "hint");
    if let OutcomeStatus::Warn { msg } = &o.status {
        assert_eq!(msg, "hint");
    } else {
        panic!("expected Warn");
    }
}
