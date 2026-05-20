//! Activation: materialize a namespace's files into a project.
//!
//! Phase 1 supports one adapter at a time with the simplest set of cases:
//! file doesn't exist in project -> symlink; file exists and differs ->
//! back up then symlink (Task 9); file exists and is byte-identical ->
//! leave in place and mark managed (Task 9). Activation failure rolls
//! back any partial materialization (Task 10).

use crate::adapter::AdapterRegistry;
use crate::atomicity::probe_rename_atomicity;
use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::manifest::AenvManifest;
use crate::state::{ActivationState, ManagedFile, MaterializeStrategy, CURRENT_SCHEMA_VERSION};
use std::path::{Path, PathBuf};

/// Activate `namespace_name` into `project_root`. Writes a state file at
/// `<project>/.aenv/state.json` on success.
pub fn activate_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    namespace_name: &str,
) -> Result<ActivationState> {
    let manifest = load_manifest(fs, layout, namespace_name)?;

    // Every adapter named in the manifest must be installed.
    for adapter_name in manifest.adapters.keys() {
        if adapters.get(adapter_name).is_none() {
            return Err(AenvError::AdapterMissing(adapter_name.clone()));
        }
    }

    // Probe rename atomicity before doing anything irreversible.
    probe_rename_atomicity(fs, project_root)?;

    let mut managed_files = Vec::new();

    for (adapter_name, entry) in &manifest.adapters {
        let adapter = adapters.get(adapter_name).expect("checked above");
        for rel in adapter_files_for_entry(adapter, entry) {
            let source = layout.namespace_dir(namespace_name).join(&rel);
            // If the namespace doesn't ship this file, skip silently.
            if !fs.exists(&source)? {
                continue;
            }
            let project_path = project_root.join(&rel);
            // Phase 1: file doesn't exist in project -> symlink.
            // Phases 9/10 add the displaced + identical paths.
            fs.symlink(&source, &project_path)?;
            managed_files.push(ManagedFile {
                path: PathBuf::from(rel),
                strategy: MaterializeStrategy::Symlink,
                source: Some(source),
            });
        }
    }

    let state = ActivationState {
        schema_version: CURRENT_SCHEMA_VERSION,
        active_namespace: namespace_name.to_string(),
        project_root: project_root.to_path_buf(),
        managed_files,
        backed_up: Vec::new(),
    };
    fs.write(
        &project_root.join(".aenv/state.json"),
        state.to_json()?.as_bytes(),
    )?;
    Ok(state)
}

fn load_manifest<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    name: &str,
) -> Result<AenvManifest> {
    let path = layout.manifest_path(name);
    if !fs.exists(&path)? {
        return Err(AenvError::NamespaceNotFound(name.to_string()));
    }
    let bytes = fs.read(&path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", path.display())))?;
    AenvManifest::from_toml(text)
}

/// Compute the set of project-relative files an adapter manages for a given
/// manifest entry. Phase 1 just intersects the adapter's `files` with the
/// entry's `files`: a path managed by the adapter is materialized only if
/// the manifest also lists it.
fn adapter_files_for_entry(
    adapter: &crate::adapter::Adapter,
    entry: &crate::manifest::AdapterEntry,
) -> Vec<String> {
    let mut out = Vec::new();
    for f in &entry.files {
        if adapter
            .files
            .iter()
            .any(|af| af == f || file_under_prefix(f, af))
        {
            out.push(f.clone());
        }
    }
    out
}

/// Whether `file` is a relative path under the directory `prefix` (which
/// ends in `/`). Adapters declare directory prefixes like `.claude/` to
/// mean "everything under this path."
fn file_under_prefix(file: &str, prefix: &str) -> bool {
    if !prefix.ends_with('/') {
        return false;
    }
    file.starts_with(prefix)
}
