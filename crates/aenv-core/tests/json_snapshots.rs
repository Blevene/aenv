//! Insta snapshot tests locking the shape of every --json response.
//!
//! Snapshots live under `tests/snapshots/`. Approve schema changes with
//! `cargo insta accept --all` and review the diff in code review.

use aenv_core::json::adapter::{AdapterEntryJson, AdapterParameterJson};
use aenv_core::json::list::ListEntry;
use aenv_core::json::skill::SkillEntry;
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

#[test]
fn list_entry_shape_is_stable() {
    let e = ListEntry {
        name: "leaf".into(),
        extends: vec!["base".into()],
        adapters: vec!["claude-code".into()],
        parameters_declared: vec!["default_model".into()],
        policies_declared: vec!["skill_requires_description".into()],
        resolved_hash: Some("sha256-v1:abc".into()),
        resolved_hash_v2: None,
        error: None,
    };
    insta::assert_json_snapshot!(e);
}

#[test]
fn adapter_entry_shape_is_stable() {
    let e = AdapterEntryJson {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into(), ".claude/skills/**/*".into()],
        skills_dir: Some(".claude/skills".into()),
        parameters: vec![AdapterParameterJson {
            name: "default_model".into(),
            type_: "string".into(),
            projects_to: None,
        }],
        soft_limits: [("instructions".to_string(), 5000)].into_iter().collect(),
    };
    insta::assert_json_snapshot!(e);
}

#[test]
fn skill_entry_shape_is_stable() {
    let e = SkillEntry {
        namespace: "leaf".into(),
        qualified_name: "leaf::write-tests".into(),
        short_name: "write-tests".into(),
        adapter: Some("claude-code".into()),
        mode: "imported".into(),
        source: Some("git+https://example.com/skills.git#write-tests".into()),
        pin: Some("v1.2.0".into()),
        required: true,
    };
    insta::assert_json_snapshot!(e);
}

#[test]
fn which_report_shape_is_stable() {
    use aenv_core::json::WhichReport;
    let r = WhichReport {
        path: std::path::PathBuf::from(".claude/skills/write-tests/SKILL.md"),
        qualified_name: "leaf::write-tests".into(),
        short_name: "write-tests".into(),
        provided_by_namespace: Some("leaf".into()),
        strategy: "symlink".into(),
        merge_kind: None,
        contributors: vec![],
        shadows: vec!["base::write-tests".into()],
    };
    insta::assert_json_snapshot!(r);
}

#[test]
fn get_report_shape_is_stable() {
    use aenv_core::json::get::{GetReport, InheritanceEntry};
    let r = GetReport {
        parameter: "default_model".into(),
        value: serde_json::json!("claude-opus-4.7"),
        source_namespace: "leaf".into(),
        inheritance_chain: vec![
            InheritanceEntry {
                namespace: "base".into(),
                value: serde_json::json!("claude-sonnet-4.6"),
            },
            InheritanceEntry {
                namespace: "leaf".into(),
                value: serde_json::json!("claude-opus-4.7"),
            },
        ],
    };
    insta::assert_json_snapshot!(r);
}

#[test]
fn doctor_report_shape_is_stable() {
    use aenv_core::json::doctor::{DoctorReportJson, OutcomeJson};
    let r = DoctorReportJson {
        namespace: "leaf".into(),
        chain: vec!["base".into(), "leaf".into()],
        policies: Default::default(),
        outcomes: vec![OutcomeJson {
            key: "instructions_max_chars".into(),
            status: "fail".into(),
            target: Some("leaf::CLAUDE.md".into()),
            msg: Some("CLAUDE.md is 5200 chars, limit 5000".into()),
        }],
        pass_count: 2,
        warn_count: 0,
        fail_count: 1,
        skipped_count: 0,
    };
    insta::assert_json_snapshot!(r);
}

#[test]
fn drift_report_shape_is_stable() {
    use aenv_core::json::diff::{DriftReport, DriftedFile};
    let r = DriftReport {
        project: std::path::PathBuf::from("/proj"),
        active_namespace: "leaf".into(),
        drifted: vec![DriftedFile {
            path: std::path::PathBuf::from("CLAUDE.md"),
            qualified_name: "leaf::CLAUDE.md".into(),
            kind: "symlink-replaced".into(),
            summary: Some("420 bytes on disk vs 380 bytes expected".into()),
        }],
    };
    insta::assert_json_snapshot!(r);
}

#[test]
fn structural_diff_shape_is_stable() {
    use aenv_core::json::diff::*;
    let d = StructuralDiff {
        a: "alpha".into(),
        b: "beta".into(),
        skills: SetDiff {
            added: vec!["beta::write-tests".into()],
            removed: vec!["alpha::quick-prototype".into()],
            // `common` is structurally always empty for skills: manifest_skill_qnames
            // emits "<ns>::<name>" strings, so two different namespaces can never share
            // an identical qualified name and the set intersection is always ∅.
            common: vec![],
        },
        agents: SetDiff::default(),
        parameters: ValueDiff {
            added: vec![],
            removed: vec![],
            changed: vec![ValueChange {
                name: "default_model".into(),
                a: serde_json::json!("claude-sonnet-4.6"),
                b: serde_json::json!("claude-opus-4.7"),
            }],
        },
        policies: ValueDiff::default(),
        instructions_sections: SetDiff::default(),
    };
    insta::assert_json_snapshot!(d);
}

#[test]
fn skill_entry_omits_adapter_when_none() {
    let e = aenv_core::json::SkillEntry {
        namespace: "leaf".into(),
        qualified_name: "leaf::stub".into(),
        short_name: "stub".into(),
        adapter: None,
        mode: "authored".into(),
        source: None,
        pin: None,
        required: false,
    };
    insta::assert_json_snapshot!(e);
}
