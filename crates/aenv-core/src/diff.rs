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
use crate::json::diff::{DriftReport, DriftedFile};
use crate::materialize::compute_material_set;
use crate::resolve::MaterializeStrategy;
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
