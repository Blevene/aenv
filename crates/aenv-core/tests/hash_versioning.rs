//! R-85 / R-87: emitted string always carries the algorithm-version prefix.

use aenv_core::hash::{hash_resolved_namespace, HASH_PREFIX_V1};
use aenv_core::materialize::MaterialSet;
use std::collections::BTreeMap;

#[test]
fn prefix_is_sha256_v1() {
    assert_eq!(HASH_PREFIX_V1, "sha256-v1:");
}

#[test]
fn emitted_strings_carry_v1_prefix() {
    let mat = MaterialSet::new(vec![], BTreeMap::new());
    let h = hash_resolved_namespace(&mat);
    assert!(
        h.starts_with("sha256-v1:"),
        "hash {h} must start with sha256-v1:"
    );
}
