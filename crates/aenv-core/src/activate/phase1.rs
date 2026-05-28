//! Phase 1 symlink materialization helper.
//!
//! Lifted from `perform_activation` so Phase 2's `materialize_one` can
//! delegate to it for `Symlink` and `Identical` strategies while Phase 2
//! handles `SectionMerge` and `DeepMerge` directly.
//!
//! `materialize_copy` here mirrors `materialize_symlink` but writes a regular
//! file (copy of the source bytes) rather than a symlink. The displaced /
//! backup logic is the same.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::identity::{NamespaceId, QualifiedName, ShortName};
use crate::state::{BackedUpFile, ManagedFile, MaterializeStrategy, SkillProvenance};
use std::path::Path;

use super::UndoStep;

/// Materialize one file via symlink (or recognize it as Identical).
///
/// Reads `classify_project_path` internally, pushes `ManagedFile` + undo
/// steps + backup records as Phase 1 did. The caller does NOT push its own
/// `ManagedFile` — this helper owns the strategy choice.
#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_symlink<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    backup_root: &Path,
    project_path: &Path,
    source: &Path,
    namespace: &NamespaceId,
    short: &ShortName,
    rel: &Path,
    shadows: Vec<QualifiedName>,
    skill_provenance: Option<SkillProvenance>,
    undo_log: &mut Vec<UndoStep>,
    managed: &mut Vec<ManagedFile>,
    backed_up: &mut Vec<BackedUpFile>,
) -> Result<()> {
    let action = super::classify_project_path(fs, project_path, source)?;
    match action {
        super::ProjectPathState::Absent => {
            if let Some(parent) = project_path.parent() {
                fs.create_dir_all(parent)?;
            }
            fs.symlink(source, project_path)?;
            undo_log.push(UndoStep::RemoveSymlink {
                link: project_path.to_path_buf(),
            });
            managed.push(ManagedFile {
                path: rel.to_path_buf(),
                qualified_name: QualifiedName::new(namespace.clone(), short.clone()),
                strategy: MaterializeStrategy::Symlink,
                contributors: vec![],
                shadows,
                skill_provenance: skill_provenance.clone(),
                was_present_before_activation: false,
            });
        }
        super::ProjectPathState::AlreadyOurSymlink => {
            managed.push(ManagedFile {
                path: rel.to_path_buf(),
                qualified_name: QualifiedName::new(namespace.clone(), short.clone()),
                strategy: MaterializeStrategy::Symlink,
                contributors: vec![],
                shadows,
                skill_provenance: skill_provenance.clone(),
                was_present_before_activation: true,
            });
        }
        super::ProjectPathState::ByteIdenticalRegular => {
            managed.push(ManagedFile {
                path: rel.to_path_buf(),
                qualified_name: QualifiedName::new(namespace.clone(), short.clone()),
                strategy: MaterializeStrategy::Identical,
                contributors: vec![],
                shadows,
                skill_provenance: skill_provenance.clone(),
                was_present_before_activation: true,
            });
        }
        super::ProjectPathState::Displaced => {
            // Refuse to clobber an existing backup file at the target.
            if fs.exists(backup_root)? {
                let backup_for_file = backup_root.join(
                    project_path
                        .strip_prefix(project_root)
                        .unwrap_or(project_path),
                );
                if fs.exists(&backup_for_file)? {
                    return Err(AenvError::ActivationConflict(format!(
                        "backup path already exists: {}",
                        backup_for_file.display()
                    )));
                }
            }
            let backup_path = backup_root.join(
                project_path
                    .strip_prefix(project_root)
                    .unwrap_or(project_path),
            );
            if let Some(parent) = backup_path.parent() {
                fs.create_dir_all(parent)?;
            }
            fs.rename(project_path, &backup_path)?;
            undo_log.push(UndoStep::RestoreBackup {
                original: project_path.to_path_buf(),
                backup: backup_path.clone(),
            });
            fs.symlink(source, project_path)?;
            undo_log.push(UndoStep::RemoveSymlink {
                link: project_path.to_path_buf(),
            });
            backed_up.push(BackedUpFile {
                original_path: project_path
                    .strip_prefix(project_root)
                    .unwrap_or(project_path)
                    .to_path_buf(),
                backup_path,
            });
            managed.push(ManagedFile {
                path: rel.to_path_buf(),
                qualified_name: QualifiedName::new(namespace.clone(), short.clone()),
                strategy: MaterializeStrategy::Symlink,
                contributors: vec![],
                shadows,
                skill_provenance,
                was_present_before_activation: true,
            });
        }
    }
    Ok(())
}

/// Materialize one file by copying source bytes to a regular file at the
/// target path.
///
/// Mirrors `materialize_symlink` for displaced / backup semantics: a
/// pre-existing target is stashed under `backup_root`, then the source bytes
/// are written. The `ByteIdenticalRegular` shortcut emits a `ManagedFile`
/// with strategy `Identical` and skips the write — same fast path as
/// symlink mode, since the on-disk content already matches.
///
/// **Why Copy is risky:** unlike Symlink (where edits write through to the
/// namespace source), Copy decouples the target from the source. The user
/// can edit the materialized file in place, but those edits are lost on
/// the next activation when the source bytes are re-copied. Task 22's
/// doctor check warns about this case.
#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_copy<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    backup_root: &Path,
    project_path: &Path,
    source: &Path,
    namespace: &NamespaceId,
    short: &ShortName,
    rel: &Path,
    shadows: Vec<QualifiedName>,
    skill_provenance: Option<SkillProvenance>,
    undo_log: &mut Vec<UndoStep>,
    managed: &mut Vec<ManagedFile>,
    backed_up: &mut Vec<BackedUpFile>,
) -> Result<()> {
    let action = super::classify_project_path(fs, project_path, source)?;
    match action {
        super::ProjectPathState::Absent => {
            if let Some(parent) = project_path.parent() {
                fs.create_dir_all(parent)?;
            }
            let source_bytes = fs.read(source)?;
            fs.write(project_path, &source_bytes)?;
            undo_log.push(UndoStep::RemoveRegularFile {
                path: project_path.to_path_buf(),
            });
            managed.push(ManagedFile {
                path: rel.to_path_buf(),
                qualified_name: QualifiedName::new(namespace.clone(), short.clone()),
                strategy: MaterializeStrategy::Copy,
                contributors: vec![],
                shadows,
                skill_provenance,
                was_present_before_activation: false,
            });
        }
        super::ProjectPathState::ByteIdenticalRegular => {
            // Target already byte-identical to source — same fast-path as the
            // Symlink branch: record as Identical, no write, no backup.
            managed.push(ManagedFile {
                path: rel.to_path_buf(),
                qualified_name: QualifiedName::new(namespace.clone(), short.clone()),
                strategy: MaterializeStrategy::Identical,
                contributors: vec![],
                shadows,
                skill_provenance,
                was_present_before_activation: true,
            });
        }
        super::ProjectPathState::AlreadyOurSymlink | super::ProjectPathState::Displaced => {
            // Refuse to clobber an existing backup file at the target.
            if fs.exists(backup_root)? {
                let backup_for_file = backup_root.join(
                    project_path
                        .strip_prefix(project_root)
                        .unwrap_or(project_path),
                );
                if fs.exists(&backup_for_file)? {
                    return Err(AenvError::ActivationConflict(format!(
                        "backup path already exists: {}",
                        backup_for_file.display()
                    )));
                }
            }
            let backup_path = backup_root.join(
                project_path
                    .strip_prefix(project_root)
                    .unwrap_or(project_path),
            );
            if let Some(parent) = backup_path.parent() {
                fs.create_dir_all(parent)?;
            }
            fs.rename(project_path, &backup_path)?;
            undo_log.push(UndoStep::RestoreBackup {
                original: project_path.to_path_buf(),
                backup: backup_path.clone(),
            });
            if let Some(parent) = project_path.parent() {
                fs.create_dir_all(parent)?;
            }
            let source_bytes = fs.read(source)?;
            fs.write(project_path, &source_bytes)?;
            undo_log.push(UndoStep::RemoveRegularFile {
                path: project_path.to_path_buf(),
            });
            backed_up.push(BackedUpFile {
                original_path: project_path
                    .strip_prefix(project_root)
                    .unwrap_or(project_path)
                    .to_path_buf(),
                backup_path,
            });
            managed.push(ManagedFile {
                path: rel.to_path_buf(),
                qualified_name: QualifiedName::new(namespace.clone(), short.clone()),
                strategy: MaterializeStrategy::Copy,
                contributors: vec![],
                shadows,
                skill_provenance,
                was_present_before_activation: true,
            });
        }
    }
    Ok(())
}
