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
    DriftReport, DriftedFile, NamedValue, SectionDelta, SetDiff, StructuralDiff, ValueChange,
    ValueDiff,
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
        .entries()
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

    let a_section_map = instruction_section_bodies(fs, layout, adapters, &a_id)?;
    let b_section_map = instruction_section_bodies(fs, layout, adapters, &b_id)?;

    let a_headings: Vec<String> = a_section_map.iter().map(|(h, _)| h.clone()).collect();
    let b_headings: Vec<String> = b_section_map.iter().map(|(h, _)| h.clone()).collect();
    let instructions_sections = set_diff(&a_headings, &b_headings);

    // Build per-section body deltas for sections common to both.
    let a_map: std::collections::BTreeMap<&str, &str> = a_section_map
        .iter()
        .map(|(h, body)| (h.as_str(), body.as_str()))
        .collect();
    let b_map: std::collections::BTreeMap<&str, &str> = b_section_map
        .iter()
        .map(|(h, body)| (h.as_str(), body.as_str()))
        .collect();
    let mut instructions_section_diffs: Vec<SectionDelta> = Vec::new();
    for heading in &instructions_sections.common {
        let a_body = a_map.get(heading.as_str()).copied().unwrap_or("");
        let b_body = b_map.get(heading.as_str()).copied().unwrap_or("");
        if a_body == b_body {
            instructions_section_diffs.push(SectionDelta {
                heading: heading.clone(),
                status: "identical".to_string(),
                summary: None,
            });
        } else {
            instructions_section_diffs.push(SectionDelta {
                heading: heading.clone(),
                status: "differs".to_string(),
                summary: Some(format!(
                    "{a}: {} chars; {b}: {} chars",
                    a_body.len(),
                    b_body.len()
                )),
            });
        }
    }

    Ok(StructuralDiff {
        a: a.to_string(),
        b: b.to_string(),
        skills,
        agents,
        parameters,
        policies,
        instructions_sections,
        instructions_section_diffs,
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

/// Parse instruction files and return a sorted, deduplicated list of
/// `(heading, body)` pairs. The body is the text between this `## ` heading
/// and the next one (or EOF), with leading/trailing whitespace trimmed.
/// When the same heading appears more than once, the last occurrence wins.
fn instruction_section_bodies<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    id: &NamespaceId,
) -> Result<Vec<(String, String)>> {
    let mat = compute_material_set(fs, layout, adapters, id)?;
    // Use a BTreeMap so that duplicate headings are deduplicated (last write
    // wins) and the result is already in sorted order.
    let mut map: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for (path, content) in mat.entries() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let is_instructions = name == "claude.md" || name.ends_with(".mdc");
        if !is_instructions {
            continue;
        }
        let Ok(s) = std::str::from_utf8(content) else {
            continue;
        };
        // Walk lines, collecting bodies per heading.
        let mut current_heading: Option<String> = None;
        let mut current_body: Vec<&str> = Vec::new();
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("## ") {
                // Flush the previous section.
                if let Some(h) = current_heading.take() {
                    map.insert(h, current_body.join("\n").trim().to_string());
                }
                current_heading = Some(rest.trim().to_string());
                current_body = Vec::new();
            } else if current_heading.is_some() {
                current_body.push(line);
            }
        }
        // Flush last section.
        if let Some(h) = current_heading {
            map.insert(h, current_body.join("\n").trim().to_string());
        }
    }
    Ok(map.into_iter().collect())
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
