//! Activation: materialize a namespace's files into a project.
//!
//! Phase 1 supports one adapter at a time. The four project-path classifications
//! (Absent / AlreadyOurSymlink / ByteIdenticalRegular / Displaced) are handled
//! in `perform_activation`; every reversible operation pushes an UndoStep
//! onto a log. On error, the log is replayed in reverse (best-effort) before
//! the error bubbles, so partial activations leave the project as we found it
//! (R-63).

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
/// `<project>/.aenv-state/state.json` on success.
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

    let mut undo_log: Vec<UndoStep> = Vec::new();
    let result = perform_activation(
        fs,
        layout,
        adapters,
        project_root,
        namespace_name,
        &manifest,
        &mut undo_log,
    );
    match result {
        Ok(state) => Ok(state),
        Err(e) => {
            undo(fs, undo_log);
            Err(e)
        }
    }
}

enum UndoStep {
    /// Created a symlink at `link`; undo by removing it.
    RemoveSymlink { link: PathBuf },
    /// Backed up `original` to `backup`; undo by renaming `backup` -> `original`.
    RestoreBackup { original: PathBuf, backup: PathBuf },
}

fn undo<F: Filesystem>(fs: &F, log: Vec<UndoStep>) {
    // Replay in reverse; best-effort (we're already in an error path, so
    // we can't recursively bail on a failed undo step).
    for step in log.into_iter().rev() {
        match step {
            UndoStep::RemoveSymlink { link } => {
                let _ = fs.remove_file(&link);
            }
            UndoStep::RestoreBackup { original, backup } => {
                let _ = fs.rename(&backup, &original);
            }
        }
    }
}

fn perform_activation<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    namespace_name: &str,
    manifest: &AenvManifest,
    undo_log: &mut Vec<UndoStep>,
) -> Result<ActivationState> {
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
                    undo_log.push(UndoStep::RemoveSymlink {
                        link: project_path.clone(),
                    });
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
                    let backup_rel =
                        PathBuf::from(format!(".aenv-state/backup/{timestamp}")).join(&rel);
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
                    undo_log.push(UndoStep::RestoreBackup {
                        original: project_path.clone(),
                        backup: backup_path.clone(),
                    });
                    fs.symlink(&source, &project_path)?;
                    undo_log.push(UndoStep::RemoveSymlink {
                        link: project_path.clone(),
                    });
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
        &project_root.join(".aenv-state/state.json"),
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
    Absent,
    AlreadyOurSymlink,
    ByteIdenticalRegular,
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
        return Ok(ProjectPathState::Displaced);
    }
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
fn backup_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("epoch-{nanos}")
}

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

fn file_under_prefix(file: &str, prefix: &str) -> bool {
    if !prefix.ends_with('/') {
        return false;
    }
    file.starts_with(prefix)
}
