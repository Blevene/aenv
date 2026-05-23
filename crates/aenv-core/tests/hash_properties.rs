//! R-81 / R-86 invariants verified with proptest.

use aenv_core::hash::hash_resolved_namespace;
use aenv_core::materialize::MaterialSet;
use aenv_core::parameters::{ParameterValue, ResolvedParameter};
use proptest::collection::vec;
use proptest::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn entry_strategy() -> impl Strategy<Value = (PathBuf, Vec<u8>)> {
    let path = "[a-z][a-z0-9_/]{0,32}\\.[a-z]{1,4}";
    (path, vec(any::<u8>(), 0..256)).prop_map(|(p, c)| (PathBuf::from(p), c))
}

fn material_set_strategy() -> impl Strategy<Value = MaterialSet> {
    vec(entry_strategy(), 0..8).prop_map(|mut entries| {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries.dedup_by(|a, b| a.0 == b.0);
        MaterialSet {
            entries,
            parameters: BTreeMap::new(),
        }
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]

    /// Order independence: reversing entries does not change the hash.
    #[test]
    fn hash_is_order_independent(mat in material_set_strategy()) {
        let mut shuffled = mat.entries.clone();
        shuffled.reverse();
        let shuffled_mat = MaterialSet {
            entries: shuffled,
            parameters: mat.parameters.clone(),
        };
        prop_assert_eq!(
            hash_resolved_namespace(&mat),
            hash_resolved_namespace(&shuffled_mat)
        );
    }

    /// Avalanche: a single-byte content flip changes the hash.
    #[test]
    fn hash_changes_on_single_byte_content_flip(
        mat in material_set_strategy().prop_filter("non-empty", |m| !m.entries.is_empty())
    ) {
        let original = hash_resolved_namespace(&mat);
        let mut flipped = mat.clone();
        if flipped.entries[0].1.is_empty() {
            flipped.entries[0].1.push(0);
        } else {
            flipped.entries[0].1[0] ^= 0x01;
        }
        prop_assert_ne!(original, hash_resolved_namespace(&flipped));
    }

    /// Any path rename changes the hash.
    #[test]
    fn hash_changes_on_path_rename(
        mat in material_set_strategy().prop_filter("non-empty", |m| !m.entries.is_empty())
    ) {
        let original = hash_resolved_namespace(&mat);
        let mut renamed = mat.clone();
        renamed.entries[0].0 = PathBuf::from(format!(
            "renamed_{}",
            renamed.entries[0].0.display()
        ));
        prop_assert_ne!(original, hash_resolved_namespace(&renamed));
    }

    /// Case sensitivity in paths.
    #[test]
    fn hash_is_path_case_sensitive(content in vec(any::<u8>(), 0..32)) {
        let lower = MaterialSet {
            entries: vec![(PathBuf::from("foo.md"), content.clone())],
            parameters: BTreeMap::new(),
        };
        let upper = MaterialSet {
            entries: vec![(PathBuf::from("FOO.md"), content)],
            parameters: BTreeMap::new(),
        };
        prop_assert_ne!(hash_resolved_namespace(&lower), hash_resolved_namespace(&upper));
    }

    /// Adding a parameter value changes the hash (via synthetic
    /// .aenv/parameters.json).
    #[test]
    fn parameter_change_changes_hash(entries in vec(entry_strategy(), 0..4)) {
        let mut sorted = entries;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        sorted.dedup_by(|a, b| a.0 == b.0);
        let no_params = MaterialSet {
            entries: sorted.clone(),
            parameters: BTreeMap::new(),
        };
        let mut params = BTreeMap::new();
        params.insert(
            "default_model".to_string(),
            ResolvedParameter {
                value: ParameterValue::String("claude-opus-4.7".into()),
                source: aenv_core::identity::NamespaceId::new("leaf").unwrap(),
            },
        );
        let with_params = MaterialSet {
            entries: sorted,
            parameters: params,
        };
        prop_assert_ne!(
            hash_resolved_namespace(&no_params),
            hash_resolved_namespace(&with_params)
        );
    }

    /// Parameter SOURCE provenance is NOT hashed — only effective values are.
    #[test]
    fn hash_ignores_parameter_provenance(value in "[a-z]{1,20}") {
        let mut params_a = BTreeMap::new();
        params_a.insert(
            "default_model".to_string(),
            ResolvedParameter {
                value: ParameterValue::String(value.clone()),
                source: aenv_core::identity::NamespaceId::new("a").unwrap(),
            },
        );
        let mut params_b = BTreeMap::new();
        params_b.insert(
            "default_model".to_string(),
            ResolvedParameter {
                value: ParameterValue::String(value),
                source: aenv_core::identity::NamespaceId::new("b").unwrap(),
            },
        );
        let a = MaterialSet { entries: vec![], parameters: params_a };
        let b = MaterialSet { entries: vec![], parameters: params_b };
        prop_assert_eq!(hash_resolved_namespace(&a), hash_resolved_namespace(&b));
    }
}
