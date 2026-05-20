//! Namespace registry operations: create, list, delete.
//!
//! These are pure library operations against a `Filesystem` and a
//! `RegistryLayout`. The CLI layer wires them to `aenv create / list /
//! delete`. Each function takes absolute paths via the layout — no env-var
//! reads, no current-dir reads.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::manifest::AenvManifest;

/// Create a new namespace by writing a default manifest. Errors if a
/// manifest already exists for `name` (PRD R-5).
pub fn create_namespace<F: Filesystem>(fs: &F, layout: &RegistryLayout, name: &str) -> Result<()> {
    let manifest_path = layout.manifest_path(name);
    if fs.exists(&manifest_path)? {
        return Err(AenvError::ManifestInvalid(format!(
            "namespace '{name}' already exists"
        )));
    }
    let manifest = AenvManifest::default_for(name);
    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    Ok(())
}

/// List every namespace in the registry. A namespace is any directory
/// under `envs/` that contains an `aenv.toml`. Returns names sorted
/// lexicographically.
pub fn list_namespaces<F: Filesystem>(fs: &F, layout: &RegistryLayout) -> Result<Vec<String>> {
    let envs_dir = layout.namespaces_dir();
    if !fs.exists(&envs_dir)? {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs.list_dir(&envs_dir)? {
        let name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        let Some(name) = name else { continue };
        if fs.exists(&layout.manifest_path(&name))? {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Delete a namespace. Errors if the namespace does not exist
/// (`NamespaceNotFound`, exit 10).
///
/// Note: PRD R-4 requires checking that the namespace is not currently
/// active in any tracked project. Phase 1 lacks a project-tracking
/// registry, so this safety net is best-effort — the CLI layer will warn
/// users that delete is destructive.
pub fn delete_namespace<F: Filesystem>(fs: &F, layout: &RegistryLayout, name: &str) -> Result<()> {
    let dir = layout.namespace_dir(name);
    if !fs.exists(&dir)? {
        return Err(AenvError::NamespaceNotFound(name.to_string()));
    }
    fs.remove_dir_all(&dir)?;
    Ok(())
}
