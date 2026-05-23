//! Insta snapshot tests locking the shape of every --json response.
//!
//! Snapshots live under `tests/snapshots/`. Approve schema changes with
//! `cargo insta accept --all` and review the diff in code review.
//!
//! Two layers of protection:
//! - Literal-construction snapshots (Tasks 8-14): catch struct-shape regressions
//!   (renamed fields, added/removed fields in the JSON schema).
//! - Builder-driven snapshots (Phase 5 follow-up 4): catch builder regressions
//!   (wrong field projections, missing strategy variants in From impls, etc.).

use aenv_core::json::adapter::{AdapterEntryJson, AdapterParameterJson};
use aenv_core::json::get::InheritanceEntry;
use aenv_core::json::list::ListEntry;
use aenv_core::json::skill::SkillEntry;
use aenv_core::json::status::{ManagedFileJson, ParameterEntryJson, StatusReport};
use std::path::PathBuf;

// ---- Shared fixture helpers ----

/// Write `contents` to `path`, creating parent directories as needed.
fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

/// Build a small 2-namespace registry in a tempdir.
///
/// Returns `(TempDir, RegistryLayout, AdapterRegistry)`.
/// - `base`: declares `CLAUDE.md` and a string parameter `default_model`.
/// - `leaf`: extends `base`, overrides `default_model`, adds its own `CLAUDE.md` section.
///
/// The `TempDir` must be kept alive for the duration of the test (drop it last).
fn make_basic_layout() -> (
    tempfile::TempDir,
    aenv_core::home::RegistryLayout,
    aenv_core::adapter::AdapterRegistry,
) {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();

    write_file(
        &layout.manifest_path("base"),
        "name = \"base\"\n\
         [adapters.claude-code]\n\
         files = [\"CLAUDE.md\"]\n\
         [parameters]\n\
         default_model = \"claude-sonnet-4.6\"\n",
    );
    write_file(
        &layout.namespace_dir("base").join("CLAUDE.md"),
        "## Project Facts\nBase facts.\n",
    );

    write_file(
        &layout.manifest_path("leaf"),
        "name = \"leaf\"\n\
         extends = [\"base\"]\n\
         [adapters.claude-code]\n\
         files = [\"CLAUDE.md\"]\n\
         [parameters]\n\
         default_model = \"claude-opus-4.7\"\n",
    );
    write_file(
        &layout.namespace_dir("leaf").join("CLAUDE.md"),
        "## Disposition\nLeaf disposition.\n",
    );

    (tmp, layout, adapters)
}

#[test]
fn status_report_shape_is_stable() {
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "default_model".into(),
        ParameterEntryJson {
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
        },
    );
    let report = StatusReport {
        project: PathBuf::from("/proj"),
        active_namespace: Some("solo".into()),
        resolution_chain: vec!["solo".into()],
        resolved_hash: Some(
            "sha256-v1:0000000000000000000000000000000000000000000000000000000000000000".into(),
        ),
        resolved_hash_v2: None,
        parameters: params,
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

// ---- Builder-driven snapshots ----
//
// Each test runs the REAL builder against a real fixture (not struct literals).
// This catches regressions in the builder logic itself — wrong field projections,
// missing strategy variants in From impls, etc. — which the literal-construction
// snapshots above cannot detect.

#[test]
fn status_report_via_builder() {
    let (_tmp, layout, adapters) = make_basic_layout();
    let project = tempfile::TempDir::new().unwrap();
    let leaf = aenv_core::identity::NamespaceId::new("leaf").unwrap();
    let fs = aenv_core::fs::RealFilesystem;
    let state =
        aenv_core::activate::activate_namespace(&fs, &layout, &adapters, project.path(), &leaf)
            .unwrap();
    let resolution = aenv_core::resolve::resolve_namespace(&fs, &layout, &adapters, &leaf)
        .map_err(aenv_core::AenvError::from)
        .unwrap();
    let mat = aenv_core::materialize::compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();
    let hash = aenv_core::hash::hash_resolved_namespace(&mat);
    let report = StatusReport::build(
        &fs,
        &layout,
        project.path().to_path_buf(),
        &state,
        &resolution,
        hash,
    );
    insta::assert_json_snapshot!(report, {
        ".project" => "[PROJECT_ROOT]",
        ".resolved_hash" => "[HASH]",
        ".managed_files[].path" => "[PATH]",
    });
}

#[test]
fn status_report_unpinned_via_builder() {
    let report = StatusReport::unpinned(PathBuf::from("/proj"));
    insta::assert_json_snapshot!(report);
}

#[test]
fn list_entry_via_builder() {
    let (_tmp, layout, adapters) = make_basic_layout();
    let fs = aenv_core::fs::RealFilesystem;
    let entry = ListEntry::build(&fs, &layout, &adapters, "leaf");
    insta::assert_json_snapshot!(entry, {
        ".resolved_hash" => "[HASH]",
    });
}

#[test]
fn adapter_entry_json_via_builder() {
    let (_tmp, layout, _adapters) = make_basic_layout();
    let fs = aenv_core::fs::RealFilesystem;
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let claude = adapters
        .get("claude-code")
        .expect("claude-code adapter present");
    let entry = AdapterEntryJson::from_adapter(claude);
    insta::assert_json_snapshot!(entry);
}

#[test]
fn skill_entry_via_builder() {
    use aenv_core::skills::{SkillDecl, SkillMode};
    let decl = SkillDecl {
        name: "write-tests".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("git+https://example.com/skills.git#write-tests".into()),
        ref_: Some("v1.2.0".into()),
        required: true,
    };
    let entry = SkillEntry::from_decl("leaf", &decl);
    insta::assert_json_snapshot!(entry);
}

#[test]
fn which_report_via_builder() {
    use aenv_core::json::WhichReport;
    let (_tmp, layout, adapters) = make_basic_layout();
    let project = tempfile::TempDir::new().unwrap();
    let leaf = aenv_core::identity::NamespaceId::new("leaf").unwrap();
    let fs = aenv_core::fs::RealFilesystem;
    let state =
        aenv_core::activate::activate_namespace(&fs, &layout, &adapters, project.path(), &leaf)
            .unwrap();
    let mf = state
        .managed_files
        .first()
        .expect("at least one managed file");
    let report = WhichReport::from_managed_file(mf);
    insta::assert_json_snapshot!(report, {
        ".path" => "[PATH]",
    });
}

#[test]
fn get_report_via_builder() {
    use aenv_core::fs::Filesystem as _;
    use aenv_core::json::get::GetReport;
    use aenv_core::manifest::AenvManifest;
    use aenv_core::parameters::ParameterValue;
    let (_tmp, layout, adapters) = make_basic_layout();
    let fs = aenv_core::fs::RealFilesystem;
    let leaf = aenv_core::identity::NamespaceId::new("leaf").unwrap();
    let rr = aenv_core::resolve::resolve_namespace(&fs, &layout, &adapters, &leaf)
        .map_err(aenv_core::AenvError::from)
        .unwrap();
    let rp = rr
        .parameters
        .get("default_model")
        .expect("default_model present");
    // Build inheritance chain by reading each namespace's manifest parameters.
    let mut inheritance: Vec<(String, ParameterValue)> = Vec::new();
    for ns in &rr.chain {
        let bytes = fs.read(&layout.manifest_path(ns.as_str())).ok();
        if let Some(b) = bytes {
            if let Ok(text) = String::from_utf8(b) {
                if let Ok(m) = AenvManifest::from_toml(&text) {
                    if let Some(pv) = m.parameters.get("default_model") {
                        inheritance.push((ns.as_str().to_string(), pv.clone()));
                    }
                }
            }
        }
    }
    let report = GetReport::build("default_model".to_string(), rp, inheritance);
    insta::assert_json_snapshot!(report);
}

#[test]
fn doctor_report_via_builder() {
    use aenv_core::json::doctor::DoctorReportJson;
    let (_tmp, layout, adapters) = make_basic_layout();
    let fs = aenv_core::fs::RealFilesystem;
    let leaf = aenv_core::identity::NamespaceId::new("leaf").unwrap();
    let rr = aenv_core::resolve::resolve_namespace(&fs, &layout, &adapters, &leaf)
        .map_err(aenv_core::AenvError::from)
        .unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &layout, &adapters, &rr);
    let report_json = DoctorReportJson::from_report("leaf", &report);
    insta::assert_json_snapshot!(report_json);
}

#[test]
fn drift_report_via_builder() {
    let (_tmp, layout, adapters) = make_basic_layout();
    let project = tempfile::TempDir::new().unwrap();
    let leaf = aenv_core::identity::NamespaceId::new("leaf").unwrap();
    let fs = aenv_core::fs::RealFilesystem;
    aenv_core::activate::activate_namespace(&fs, &layout, &adapters, project.path(), &leaf)
        .unwrap();
    let report = aenv_core::diff::project_drift(&fs, &layout, &adapters, project.path()).unwrap();
    insta::assert_json_snapshot!(report, {
        ".project" => "[PROJECT_ROOT]",
    });
}

#[test]
fn structural_diff_via_builder() {
    let (_tmp, layout, adapters) = make_basic_layout();
    let fs = aenv_core::fs::RealFilesystem;
    let diff = aenv_core::diff::structural(&fs, &layout, &adapters, "base", "leaf").unwrap();
    insta::assert_json_snapshot!(diff);
}
