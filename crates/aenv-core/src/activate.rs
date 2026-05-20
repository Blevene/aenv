//! Activation: materialize a namespace's files into a project.
//!
//! Phase 1 supports one adapter at a time with the simplest set of cases:
//! file doesn't exist in project -> symlink; file exists and differs ->
//! back up then symlink; file exists and is byte-identical -> leave in
//! place and mark managed. Activation failure rolls back any partial
//! materialization (Task 10).

use crate::adapter::AdapterRegistry;
use crate::atomicity::probe_rename_atomicity;
use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::manifest::AenvManifest;
use crate::state::{
    ActivationState, BackedUpFile, ManagedFile, MaterializeStrategy, CURRENT_SCHEMA_VERSION,
};
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

    let timestamp = backup_timestamp();
    let mut managed_files = Vec::new();
    let mut backed_up = Vec::new();

    for (adapter_name, entry) in &manifest.adapters {
        let adapter = adapters.get(adapter_name).expect("checked above");
        for rel in adapter_files_for_entry(adapter, entry) {
            let source = layout.namespace_dir(namespace_name).join(&rel);
            if !fs.exists(&source)? {
                continue;
            }
            let project_path = project_root.join(&rel);
            let action = classify_project_path(fs, &project_path, &source)?;
            match action {
                ProjectPathState::Absent => {
                    fs.symlink(&source, &project_path)?;
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
                ProjectPathState::AlreadyOurSymlink => {
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
                ProjectPathState::ByteIdenticalRegular => {
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Identical,
                        source: None,
                    });
                }
                ProjectPathState::Displaced => {
                    let backup_rel = PathBuf::from(format!(".aenv/backup/{timestamp}")).join(&rel);
                    let backup_path = project_root.join(&backup_rel);
                    // Refuse to clobber an existing backup file at the
                    // target — protects R-61 against nanosecond-precision
                    // collisions and against stray backup contents.
                    if fs.exists(&backup_path)? {
                        return Err(AenvError::ActivationConflict(format!(
                            "backup path already exists: {}",
                            backup_path.display()
                        )));
                    }
                    if let Some(parent) = backup_path.parent() {
                        fs.create_dir_all(parent)?;
                    }
                    fs.rename(&project_path, &backup_path)?;
                    fs.symlink(&source, &project_path)?;
                    backed_up.push(BackedUpFile {
                        original_path: PathBuf::from(&rel),
                        backup_path: backup_rel,
                    });
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
            }
        }
    }

    let state = ActivationState {
        schema_version: CURRENT_SCHEMA_VERSION,
        active_namespace: namespace_name.to_string(),
        project_root: project_root.to_path_buf(),
        managed_files,
        backed_up,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectPathState {
    /// Nothing at the project path.
    Absent,
    /// Already an aenv-managed symlink pointing at our intended source.
    AlreadyOurSymlink,
    /// Regular file whose contents match the namespace's source.
    ByteIdenticalRegular,
    /// Something exists and differs — must back up.
    Displaced,
}

/// Decide what to do with the project path before materializing the source.
///
/// **Important:** we check `symlink_metadata` BEFORE `exists` because
/// `exists` follows symlinks. A stale aenv-managed symlink (target deleted)
/// would return `Ok(false)` from `exists` and get misclassified as Absent;
/// we'd then try to create a fresh symlink on top, fail with EEXIST on real
/// fs, and have no undo entry. Checking the link itself first closes this
/// hole. (Phase 0.5 P0 bug.)
fn classify_project_path<F: Filesystem>(
    fs: &F,
    project_path: &Path,
    source: &Path,
) -> Result<ProjectPathState> {
    // Inspect the path itself, not what it points to. NotFound means
    // nothing at the path; any other error propagates.
    let meta = match fs.symlink_metadata(project_path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProjectPathState::Absent);
        }
        Err(e) => return Err(AenvError::Io(e)),
    };
    if matches!(meta.kind, crate::fs::FileKind::Symlink) {
        let target = fs.read_link(project_path)?;
        if target == source {
            return Ok(ProjectPathState::AlreadyOurSymlink);
        }
        // Stale or other-target symlink: displace it. The backup will be
        // the link itself (rename moves the link, not the target). Reading
        // the backup later dereferences to whatever the link pointed at;
        // dangling-target case behaves as it did before activation.
        return Ok(ProjectPathState::Displaced);
    }
    // Regular file: compare bytes for the identical-case shortcut.
    if matches!(meta.kind, crate::fs::FileKind::File) {
        let project_bytes = fs.read(project_path)?;
        let source_bytes = fs.read(source)?;
        if project_bytes == source_bytes {
            return Ok(ProjectPathState::ByteIdenticalRegular);
        }
    }
    Ok(ProjectPathState::Displaced)
}

/// Filesystem-safe timestamp string for backup directory names.
///
/// Uses nanosecond precision so two activations within the same wall-clock
/// second don't collide. Same-nanosecond collisions are vanishingly rare;
/// if one ever happens we still avoid silent overwrite by checking for
/// directory existence at the caller (see the `Displaced` arm).
fn backup_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("epoch-{nanos}")
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
