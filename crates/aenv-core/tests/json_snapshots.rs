//! Insta snapshot tests locking the shape of every --json response.
//!
//! Snapshots live under `tests/snapshots/`. Approve schema changes with
//! `cargo insta accept --all` and review the diff in code review.

use aenv_core::json::status::{ManagedFileJson, StatusReport};
use std::path::PathBuf;

#[test]
fn status_report_shape_is_stable() {
    let report = StatusReport {
        project: PathBuf::from("/proj"),
        active_namespace: Some("solo".into()),
        resolution_chain: vec!["solo".into()],
        resolved_hash: Some(
            "sha256-v1:0000000000000000000000000000000000000000000000000000000000000000".into(),
        ),
        resolved_hash_v2: None,
        parameters: Default::default(),
        policies: Default::default(),
        managed_files: vec![ManagedFileJson {
            path: PathBuf::from("CLAUDE.md"),
            qualified_name: "solo::CLAUDE.md".into(),
            short_name: "CLAUDE.md".into(),
            provided_by_namespace: Some("solo".into()),
            strategy: "symlink".into(),
            merge_kind: None,
            contributors: vec![],
            shadows: vec![],
            skill_provenance: None,
        }],
        backed_up: vec![],
        warnings: vec![],
    };
    insta::assert_json_snapshot!(report);
}
