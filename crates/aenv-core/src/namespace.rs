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
use std::collections::BTreeMap;

/// Create a new namespace by writing a default manifest. Errors if a
/// manifest already exists for `name` (PRD R-5).
///
/// `extends` lists parent namespaces to inherit from. Pass `&[]` for a
/// standalone namespace (the common case for `aenv create <name>` with no flag).
///
/// `adapter_names` seeds empty `[adapters.<name>]` blocks in the manifest.
/// The caller is responsible for validating that these names are installed;
/// this function trusts the slice (Option 1 / CLI-layer validation).
pub fn create_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    name: &str,
    extends: &[String],
    adapter_names: &[String],
) -> Result<()> {
    let manifest_path = layout.manifest_path(name);
    if fs.exists(&manifest_path)? {
        return Err(AenvError::ManifestInvalid(format!(
            "namespace '{name}' already exists"
        )));
    }
    let mut manifest = AenvManifest::default_for(name);
    manifest.extends = extends.to_vec();
    for adapter_name in adapter_names {
        manifest.adapters.insert(
            adapter_name.clone(),
            crate::manifest::AdapterEntry::default(),
        );
    }
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
            .map(std::string::ToString::to_string);
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

/// Create a new namespace by gathering every adapter-managed file at the
/// project root and copying it into the namespace dir.
///
/// For literal entries in `adapter.files`: copy if present, skip if absent.
///
/// For glob entries (containing `*`): derive the literal directory prefix
/// (everything before the first `*` segment), walk the project tree under
/// that prefix, and copy every regular file encountered. Symlinks are
/// followed — the bytes captured represent the project's effective harness
/// state at fork time.
///
/// The new manifest carries the *resolved literal paths* it captured, not
/// the source glob pattern.
pub fn create_namespace_from_project<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    adapters: &crate::adapter::AdapterRegistry,
    new_name: &str,
    project_root: &std::path::Path,
) -> Result<()> {
    let dest = registry.namespace_dir(new_name);
    if fs.exists(&dest)? {
        return Err(AenvError::ManifestInvalid(format!(
            "namespace {new_name} already exists at {}",
            dest.display()
        )));
    }
    let mut manifest_adapters = std::collections::BTreeMap::new();
    for (_, adapter) in adapters.iter() {
        let mut files: Vec<String> = Vec::new();
        for rel in &adapter.files {
            if rel.contains('*') {
                let prefix = literal_prefix(rel);
                let walk_root = project_root.join(prefix);
                if fs.exists(&walk_root)? {
                    let mut found: Vec<String> = Vec::new();
                    walk_project_tree(fs, project_root, &walk_root, &mut found)?;
                    for f in found {
                        let proj_path = project_root.join(&f);
                        let bytes = fs.read(&proj_path)?;
                        let dest_path = dest.join(&f);
                        fs.write(&dest_path, &bytes)?;
                        files.push(f);
                    }
                }
            } else {
                let proj_path = project_root.join(rel);
                if fs.exists(&proj_path)? {
                    let bytes = fs.read(&proj_path)?;
                    let dest_path = dest.join(rel);
                    fs.write(&dest_path, &bytes)?;
                    files.push(rel.clone());
                }
            }
        }
        files.sort();
        files.dedup();
        if !files.is_empty() {
            manifest_adapters.insert(
                adapter.name.clone(),
                crate::manifest::AdapterEntry { files, merge: None },
            );
        }
    }
    let manifest = AenvManifest {
        name: new_name.to_string(),
        extends: vec![],
        adapters: manifest_adapters,
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        skills: Vec::new(),
    };
    let body =
        toml::to_string_pretty(&manifest).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    fs.write(&registry.manifest_path(new_name), body.as_bytes())?;
    Ok(())
}

/// Return the longest literal directory prefix before the first `*` in a
/// glob pattern. For example:
/// - `.claude/skills/**/*` → `.claude/skills`
/// - `**/*`               → ``  (empty string)
/// - `foo/bar.md`         → `foo/bar.md` (no glob, returned as-is)
fn literal_prefix(pattern: &str) -> &str {
    match pattern.find('*') {
        Some(i) => {
            let candidate = &pattern[..i];
            match candidate.rfind('/') {
                Some(slash) => &pattern[..slash],
                None => "",
            }
        }
        None => pattern,
    }
}

/// Recursively walk `walk_root`, appending project-relative paths of every
/// regular file to `out`. Skips `.aenv-state` directories.
fn walk_project_tree<F: Filesystem>(
    fs: &F,
    project_root: &std::path::Path,
    walk_root: &std::path::Path,
    out: &mut Vec<String>,
) -> Result<()> {
    let mut entries = fs.list_dir(walk_root)?;
    entries.sort(); // deterministic order
    for entry in entries {
        let name = entry.file_name().map(|n| n.to_string_lossy().to_string());
        if name.as_deref() == Some(".aenv-state") {
            continue;
        }
        let meta = fs.metadata(&entry)?;
        if matches!(meta.kind, crate::fs::FileKind::Directory) {
            walk_project_tree(fs, project_root, &entry, out)?;
        } else if let Ok(rel) = entry.strip_prefix(project_root) {
            out.push(rel.to_string_lossy().to_string());
        }
    }
    Ok(())
}
