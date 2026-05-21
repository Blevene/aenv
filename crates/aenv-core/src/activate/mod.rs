//! Activation: materialize a namespace's files into a project.
//!
//! Phase 2 wires the resolver + strategy + merge primitives. The function
//! resolves the full `extends` chain, groups candidates by project-relative
//! path, decides a `MaterializeStrategy` per group, then materializes each:
//!
//! * `Symlink` / `Identical` — delegate to `phase1::materialize_symlink`.
//! * `SectionMerge` — merge all bodies as Markdown sections, write a regular file.
//! * `DeepMerge(format)` — merge structured data, write a regular file.
//!
//! Every reversible write pushes an `UndoStep`. On error the log is replayed
//! in reverse (best-effort) before the error bubbles, leaving the project as
//! we found it (R-63).

mod phase1;

use crate::adapter::AdapterRegistry;
use crate::atomicity::probe_rename_atomicity;
use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::identity::{NamespaceId, QualifiedName, ShortName};
use crate::resolve::MaterializeStrategy;
use crate::state::{ActivationState, BackedUpFile, ManagedFile, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Activate `leaf` namespace into `project_root`. Resolves the full
/// `extends` chain, merges or symlinks each managed file, and writes
/// `.aenv-state/state.json` on success.
pub fn activate_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    leaf: &NamespaceId,
) -> Result<ActivationState> {
    probe_rename_atomicity(fs, project_root)?;

    let resolution = crate::resolve::resolve_namespace(fs, layout, adapters, leaf)?;

    // Group candidates by project-relative path, preserving chain order within
    // each group. BTreeMap gives us lexicographic iteration order (deterministic
    // activation and rollback).
    let mut by_path: BTreeMap<PathBuf, Vec<crate::resolve::Candidate>> = Default::default();
    for c in resolution.candidates {
        by_path.entry(c.path.clone()).or_default().push(c);
    }

    let mut undo_log: Vec<UndoStep> = Vec::new();
    let mut managed: Vec<ManagedFile> = Vec::new();
    let mut backed_up: Vec<BackedUpFile> = Vec::new();
    let backup_root = backup_dir_for_this_run(project_root);

    let result: Result<()> = (|| {
        for (path, candidates) in &by_path {
            let strategy = crate::strategy::decide_strategy(candidates, adapters)?;
            materialize_one(
                fs,
                adapters,
                project_root,
                &backup_root,
                path,
                candidates,
                strategy,
                &mut undo_log,
                &mut managed,
                &mut backed_up,
            )?;
        }
        Ok(())
    })();

    if let Err(e) = result {
        undo(fs, std::mem::take(&mut undo_log));
        return Err(e);
    }

    let state = ActivationState {
        schema_version: SCHEMA_VERSION,
        active_namespace: leaf.as_str().to_owned(),
        project_root: project_root.to_path_buf(),
        managed_files: managed,
        backed_up,
    };
    let state_path = project_root.join(".aenv-state/state.json");
    let body = serde_json::to_vec_pretty(&state)
        .map_err(|e| AenvError::ActivationConflict(format!("state serialize: {e}")))?;
    if let Err(e) = fs.write(&state_path, &body) {
        undo(fs, std::mem::take(&mut undo_log));
        return Err(AenvError::Io(e));
    }
    Ok(state)
}

// --------------------------------------------------------------------------
// UndoStep + undo log
// --------------------------------------------------------------------------

pub(super) enum UndoStep {
    /// Created a symlink at `link`; undo by removing it.
    RemoveSymlink { link: PathBuf },
    /// Backed up `original` to `backup`; undo by renaming `backup` -> `original`.
    RestoreBackup { original: PathBuf, backup: PathBuf },
    /// Wrote a regular file at `path` (Phase 2 merge output); undo by removing it.
    RemoveRegularFile { path: PathBuf },
}

fn undo<F: Filesystem>(fs: &F, log: Vec<UndoStep>) {
    for step in log.into_iter().rev() {
        match step {
            UndoStep::RemoveSymlink { link } => {
                let _ = fs.remove_file(&link);
            }
            UndoStep::RestoreBackup { original, backup } => {
                let _ = fs.rename(&backup, &original);
            }
            UndoStep::RemoveRegularFile { path } => {
                let _ = fs.remove_file(&path);
            }
        }
    }
}

// --------------------------------------------------------------------------
// Per-path materialization dispatch
// --------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn materialize_one<F: Filesystem>(
    fs: &F,
    adapters: &AdapterRegistry,
    project_root: &Path,
    backup_root: &Path,
    path: &Path,
    candidates: &[crate::resolve::Candidate],
    strategy: MaterializeStrategy,
    undo_log: &mut Vec<UndoStep>,
    managed: &mut Vec<ManagedFile>,
    backed_up: &mut Vec<BackedUpFile>,
) -> Result<()> {
    let project_path = project_root.join(path);
    match strategy {
        MaterializeStrategy::Symlink | MaterializeStrategy::Identical => {
            let latest = candidates.last().expect("non-empty");
            let shadows = crate::shadow::compute_shadows(candidates, strategy, adapters)?;
            let qn = crate::shadow::qualified_from_candidate(latest)?;
            let short = ShortName::new(path.to_string_lossy().to_string())
                .map_err(|e| AenvError::ManifestInvalid(format!("short name: {e}")))?;
            phase1::materialize_symlink(
                fs,
                project_root,
                backup_root,
                &project_path,
                &latest.source_path,
                qn.namespace(),
                &short,
                path,
                shadows,
                undo_log,
                managed,
                backed_up,
            )?;
        }
        MaterializeStrategy::SectionMerge => {
            let bodies = read_all_as_strings(fs, candidates)?;
            let merged = crate::merge::section::merge_sections(&bodies);
            write_merged_regular(
                fs,
                project_root,
                backup_root,
                &project_path,
                merged.as_bytes(),
                undo_log,
                backed_up,
            )?;
            managed.push(ManagedFile {
                path: path.to_path_buf(),
                qualified_name: synthesize_merged_qn(path)?,
                strategy,
                contributors: candidates
                    .iter()
                    .map(crate::shadow::qualified_from_candidate)
                    .collect::<Result<Vec<_>>>()?,
                shadows: vec![],
            });
        }
        MaterializeStrategy::DeepMerge(format) => {
            let bodies = read_all_as_bytes(fs, candidates)?;
            let merged = match format {
                crate::resolve::DeepMergeFormat::Json => {
                    crate::merge::deep_json::merge_json(&bodies).map_err(AenvError::from)?
                }
                crate::resolve::DeepMergeFormat::Yaml => {
                    crate::merge::deep_yaml::merge_yaml(&bodies).map_err(AenvError::from)?
                }
                crate::resolve::DeepMergeFormat::Toml => {
                    crate::merge::deep_toml::merge_toml(&bodies).map_err(AenvError::from)?
                }
            };
            write_merged_regular(
                fs,
                project_root,
                backup_root,
                &project_path,
                &merged,
                undo_log,
                backed_up,
            )?;
            managed.push(ManagedFile {
                path: path.to_path_buf(),
                qualified_name: synthesize_merged_qn(path)?,
                strategy,
                contributors: candidates
                    .iter()
                    .map(crate::shadow::qualified_from_candidate)
                    .collect::<Result<Vec<_>>>()?,
                shadows: vec![],
            });
        }
        MaterializeStrategy::Copy => {
            return Err(AenvError::ActivationConflict(
                "Copy strategy is Phase 7 (Windows fallback); not supported in Phase 2".into(),
            ));
        }
        MaterializeStrategy::Merged => {
            return Err(AenvError::ActivationConflict(
                "Phase 1 'Merged' variant should not be produced by Phase 2".into(),
            ));
        }
    }
    Ok(())
}

// --------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------

/// Synthesize a `(merged)::path` qualified name for a merged artifact.
pub(crate) fn synthesize_merged_qn(path: &Path) -> Result<QualifiedName> {
    let short = ShortName::new(path.to_string_lossy().to_string())
        .map_err(|e| AenvError::ManifestInvalid(format!("short name: {e}")))?;
    Ok(QualifiedName::new(NamespaceId::merged_synthetic(), short))
}

fn read_all_as_bytes<F: Filesystem>(
    fs: &F,
    candidates: &[crate::resolve::Candidate],
) -> Result<Vec<Vec<u8>>> {
    candidates
        .iter()
        .map(|c| fs.read(&c.source_path).map_err(AenvError::from))
        .collect()
}

fn read_all_as_strings<F: Filesystem>(
    fs: &F,
    candidates: &[crate::resolve::Candidate],
) -> Result<Vec<String>> {
    candidates
        .iter()
        .map(|c| {
            let bytes = fs.read(&c.source_path)?;
            String::from_utf8(bytes).map_err(|e| {
                AenvError::ActivationConflict(format!(
                    "UTF-8 decode {}: {e}",
                    c.source_path.display()
                ))
            })
        })
        .collect()
}

fn write_merged_regular<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    backup_root: &Path,
    project_path: &Path,
    contents: &[u8],
    undo_log: &mut Vec<UndoStep>,
    backed_up: &mut Vec<BackedUpFile>,
) -> Result<()> {
    let existed = fs.exists(project_path)?;
    if existed {
        let rel = project_path
            .strip_prefix(project_root)
            .unwrap_or(project_path);
        let backup_path = backup_root.join(rel);
        if let Some(parent) = backup_path.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.rename(project_path, &backup_path)?;
        undo_log.push(UndoStep::RestoreBackup {
            original: project_path.to_path_buf(),
            backup: backup_path.clone(),
        });
        backed_up.push(BackedUpFile {
            original_path: rel.to_path_buf(),
            backup_path,
        });
    }
    if let Some(parent) = project_path.parent() {
        fs.create_dir_all(parent)?;
    }
    fs.write(project_path, contents)?;
    undo_log.push(UndoStep::RemoveRegularFile {
        path: project_path.to_path_buf(),
    });
    Ok(())
}

// --------------------------------------------------------------------------
// Fork primitives
// --------------------------------------------------------------------------

/// Detach a single materialized file from namespace management.
///
/// For symlinks: replace with a regular copy of the target bytes. For merged
/// files: leave on disk unchanged. In both cases: remove from
/// `state.managed_files` so subsequent activations won't touch it.
pub fn fork_file<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    rel_path: &Path,
) -> crate::error::Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    let body = fs.read(&state_path)?;
    let mut state = crate::state::ActivationState::from_json(
        std::str::from_utf8(&body).map_err(|e| AenvError::ActivationConflict(e.to_string()))?,
    )?;
    let pos = state
        .managed_files
        .iter()
        .position(|m| m.path == rel_path)
        .ok_or_else(|| {
            AenvError::ActivationConflict(format!(
                "{} is not managed by the active namespace",
                rel_path.display()
            ))
        })?;
    let project_path = project_root.join(rel_path);
    if matches!(
        state.managed_files[pos].strategy,
        MaterializeStrategy::Symlink
    ) {
        // Read through the symlink to get the underlying bytes, then replace.
        let bytes = fs.read(&project_path)?;
        fs.remove_file(&project_path)?;
        fs.write(&project_path, &bytes)?;
    }
    state.managed_files.remove(pos);
    let new_body = state.to_json()?;
    fs.write(&state_path, new_body.as_bytes())?;
    Ok(())
}

/// Detach the entire project from namespace management.
///
/// For every managed file with strategy `Symlink`, reads the resolved bytes
/// through the symlink and replaces it with a regular file. For merged
/// strategies the file is already a regular file on disk — leave it. Then
/// removes `.aenv-state/` entirely so subsequent activations and shell-hook
/// auto-activation skip this project.
///
/// The `.aenv` pin file is intentionally NOT removed — the project retains
/// its declaration of "I was forked from <namespace>" for human reference.
/// Re-pin with `aenv use <name>` to re-enable activation.
///
/// Idempotent: a project with no state file returns `Ok(())` without
/// touching anything.
pub fn fork_project<F: Filesystem>(fs: &F, project_root: &Path) -> crate::error::Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        return Ok(());
    }
    let body = fs.read(&state_path)?;
    let state = crate::state::ActivationState::from_json(
        std::str::from_utf8(&body).map_err(|e| AenvError::ActivationConflict(e.to_string()))?,
    )?;

    for mf in &state.managed_files {
        if matches!(mf.strategy, MaterializeStrategy::Symlink) {
            let project_path = project_root.join(&mf.path);
            let bytes = fs.read(&project_path)?;
            fs.remove_file(&project_path)?;
            fs.write(&project_path, &bytes)?;
        }
    }

    let state_dir = project_root.join(".aenv-state");
    fs.remove_dir_all(&state_dir)?;
    Ok(())
}

// --------------------------------------------------------------------------
// Phase 1 helpers kept for phase1.rs and the classify step
// --------------------------------------------------------------------------

/// Filesystem-safe timestamp string for backup directory names.
fn backup_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("epoch-{nanos}")
}

/// Build the backup root for this activation run.
pub(super) fn backup_dir_for_this_run(project_root: &Path) -> PathBuf {
    project_root
        .join(".aenv-state/backup")
        .join(backup_timestamp())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectPathState {
    Absent,
    AlreadyOurSymlink,
    ByteIdenticalRegular,
    Displaced,
}

/// Decide what to do with the project path before materializing the source.
///
/// Checks `symlink_metadata` BEFORE `exists` to avoid misclassifying a stale
/// symlink (whose target is gone) as Absent.
pub(super) fn classify_project_path<F: Filesystem>(
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
