//! Project-drift detection and structural namespace diff.
//!
//! `project_drift` walks `state.managed_files` and compares each entry's
//! on-disk bytes against what the current resolution would materialize.
//! `structural` (Task 13) compares two namespaces' skill rosters,
//! parameters, policies, and instructions-section headers.

use std::path::Path;

use crate::adapter::AdapterRegistry;
use crate::error::Result;
use crate::fs::{FileKind, Filesystem};
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::json::diff::{
    DriftReport, DriftedFile, NamedValue, SetDiff, StructuralDiff, ValueChange, ValueDiff,
};
use crate::manifest::AenvManifest;
use crate::materialize::compute_material_set;
use crate::parameters::{ParameterValue, ResolvedParameter};
use crate::policies::{PolicyValue, ResolvedPolicy};
use crate::resolve::{resolve_namespace, MaterializeStrategy};
use crate::state::ActivationState;

/// Detect project drift. Returns an empty `DriftReport.drifted` if the
/// project is unpinned (no `.aenv-state/state.json`) or no managed file
/// has diverged.
pub fn project_drift<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
) -> Result<DriftReport> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        return Ok(DriftReport {
            project: project_root.to_path_buf(),
            active_namespace: String::new(),
            drifted: vec![],
        });
    }
    let bytes = fs.read(&state_path)?;
    let text = String::from_utf8(bytes)
        .map_err(|e| crate::AenvError::ManifestInvalid(format!("state.json: {e}")))?;
    let state = ActivationState::from_json(&text)?;

    let leaf = NamespaceId::new(state.active_namespace.as_str())?;
    let mat = compute_material_set(fs, layout, adapters, &leaf)?;

    // Build a lookup map: project-relative path → expected bytes.
    let mut expected: std::collections::BTreeMap<&std::path::Path, &[u8]> = mat
        .entries
        .iter()
        .map(|(p, c)| (p.as_path(), c.as_slice()))
        .collect();

    let mut drifted: Vec<DriftedFile> = Vec::new();
    for mf in &state.managed_files {
        let project_path = project_root.join(&mf.path);
        let on_disk = match fs.read(&project_path) {
            Ok(b) => b,
            Err(_) => {
                drifted.push(DriftedFile {
                    path: mf.path.clone(),
                    qualified_name: mf.qualified_name.to_string(),
                    kind: "content-divergent".into(),
                    summary: Some("file missing".into()),
                });
                continue;
            }
        };

        let Some(expected_bytes) = expected.remove(mf.path.as_path()) else {
            drifted.push(DriftedFile {
                path: mf.path.clone(),
                qualified_name: mf.qualified_name.to_string(),
                kind: "content-divergent".into(),
                summary: Some("file no longer produced by resolution".into()),
            });
            continue;
        };

        if on_disk == expected_bytes {
            continue;
        }

        let kind = match mf.strategy {
            MaterializeStrategy::Symlink => match fs.symlink_metadata(&project_path) {
                Ok(meta) if matches!(meta.kind, FileKind::Symlink) => "content-divergent",
                _ => "symlink-replaced",
            },
            MaterializeStrategy::SectionMerge | MaterializeStrategy::DeepMerge(_) => {
                "merge-regenerated"
            }
            _ => "content-divergent",
        };
        drifted.push(DriftedFile {
            path: mf.path.clone(),
            qualified_name: mf.qualified_name.to_string(),
            kind: kind.to_string(),
            summary: Some(diff_summary(&on_disk, expected_bytes)),
        });
    }

    Ok(DriftReport {
        project: project_root.to_path_buf(),
        active_namespace: state.active_namespace,
        drifted,
    })
}

fn diff_summary(on_disk: &[u8], expected: &[u8]) -> String {
    let on_disk_lines = on_disk.iter().filter(|&&b| b == b'\n').count();
    let exp_lines = expected.iter().filter(|&&b| b == b'\n').count();
    format!(
        "{} bytes on disk vs {} bytes expected ({} vs {} lines)",
        on_disk.len(),
        expected.len(),
        on_disk_lines,
        exp_lines
    )
}

// ---- Task 13: structural diff ----

/// Structural diff between two namespaces. Compares their resolved
/// skills, parameters, policies, and instructions-section headers.
pub fn structural<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    a: &str,
    b: &str,
) -> Result<StructuralDiff> {
    let a_id = NamespaceId::new(a)?;
    let b_id = NamespaceId::new(b)?;
    let a_res = resolve_namespace(fs, layout, adapters, &a_id)?;
    let b_res = resolve_namespace(fs, layout, adapters, &b_id)?;

    // Skill rosters: union of skills declared in each leaf's manifest,
    // keyed by qualified short-name. Agents always empty today (no
    // [[agents]] table in manifests yet).
    let a_skills = manifest_skill_qnames(fs, layout, a)?;
    let b_skills = manifest_skill_qnames(fs, layout, b)?;
    let skills = set_diff(&a_skills, &b_skills);
    let agents = SetDiff::default();

    let parameters = value_diff_params(&a_res.parameters, &b_res.parameters);
    let policies = value_diff_policies(&a_res.policies, &b_res.policies);

    let a_sections = instruction_section_headers(fs, layout, adapters, &a_id)?;
    let b_sections = instruction_section_headers(fs, layout, adapters, &b_id)?;
    let instructions_sections = set_diff(&a_sections, &b_sections);

    Ok(StructuralDiff {
        a: a.to_string(),
        b: b.to_string(),
        skills,
        agents,
        parameters,
        policies,
        instructions_sections,
    })
}

fn manifest_skill_qnames<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    ns: &str,
) -> Result<Vec<String>> {
    let bytes = fs.read(&layout.manifest_path(ns))?;
    let text = String::from_utf8(bytes)
        .map_err(|e| crate::AenvError::ManifestInvalid(format!("manifest utf-8: {e}")))?;
    let m = AenvManifest::from_toml(&text)?;
    Ok(m.skills
        .iter()
        .map(|s| format!("{ns}::{}", s.name))
        .collect())
}

fn instruction_section_headers<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    id: &NamespaceId,
) -> Result<Vec<String>> {
    let mat = compute_material_set(fs, layout, adapters, id)?;
    let mut headers: Vec<String> = Vec::new();
    for (path, content) in &mat.entries {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let is_instructions = name == "claude.md" || name.ends_with(".mdc");
        if !is_instructions {
            continue;
        }
        if let Ok(s) = std::str::from_utf8(content) {
            for line in s.lines() {
                if let Some(rest) = line.strip_prefix("## ") {
                    headers.push(rest.trim().to_string());
                }
            }
        }
    }
    headers.sort();
    headers.dedup();
    Ok(headers)
}

fn set_diff(a: &[String], b: &[String]) -> SetDiff {
    let a_set: std::collections::BTreeSet<&str> = a.iter().map(String::as_str).collect();
    let b_set: std::collections::BTreeSet<&str> = b.iter().map(String::as_str).collect();
    SetDiff {
        added: b_set.difference(&a_set).map(|s| (*s).to_string()).collect(),
        removed: a_set.difference(&b_set).map(|s| (*s).to_string()).collect(),
        common: a_set
            .intersection(&b_set)
            .map(|s| (*s).to_string())
            .collect(),
    }
}

fn value_diff_params(
    a: &std::collections::BTreeMap<String, ResolvedParameter>,
    b: &std::collections::BTreeMap<String, ResolvedParameter>,
) -> ValueDiff {
    let to_json = |v: &ParameterValue| -> serde_json::Value {
        match v {
            ParameterValue::String(s) => serde_json::Value::String(s.clone()),
            ParameterValue::Integer(i) => serde_json::Value::Number((*i).into()),
            ParameterValue::Boolean(b) => serde_json::Value::Bool(*b),
            ParameterValue::ListString(xs) => serde_json::Value::Array(
                xs.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        }
    };
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for (k, va) in a {
        match b.get(k) {
            None => removed.push(NamedValue {
                name: k.clone(),
                value: to_json(&va.value),
            }),
            Some(vb) if vb.value != va.value => changed.push(ValueChange {
                name: k.clone(),
                a: to_json(&va.value),
                b: to_json(&vb.value),
            }),
            _ => {}
        }
    }
    for (k, vb) in b {
        if !a.contains_key(k) {
            added.push(NamedValue {
                name: k.clone(),
                value: to_json(&vb.value),
            });
        }
    }
    ValueDiff {
        added,
        removed,
        changed,
    }
}

fn value_diff_policies(
    a: &std::collections::BTreeMap<String, ResolvedPolicy>,
    b: &std::collections::BTreeMap<String, ResolvedPolicy>,
) -> ValueDiff {
    let to_json = |v: &PolicyValue| -> serde_json::Value {
        match v {
            PolicyValue::Integer(i) => serde_json::Value::Number((*i).into()),
            PolicyValue::Boolean(b) => serde_json::Value::Bool(*b),
            PolicyValue::ListString(xs) => serde_json::Value::Array(
                xs.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        }
    };
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for (k, va) in a {
        match b.get(k) {
            None => removed.push(NamedValue {
                name: k.clone(),
                value: to_json(&va.value),
            }),
            Some(vb) if vb.value != va.value => changed.push(ValueChange {
                name: k.clone(),
                a: to_json(&va.value),
                b: to_json(&vb.value),
            }),
            _ => {}
        }
    }
    for (k, vb) in b {
        if !a.contains_key(k) {
            added.push(NamedValue {
                name: k.clone(),
                value: to_json(&vb.value),
            });
        }
    }
    ValueDiff {
        added,
        removed,
        changed,
    }
}
