//! Phase 1 symlink materialization helper.
//!
//! Lifted from `perform_activation` so Phase 2's `materialize_one` can
//! delegate to it for `Symlink` and `Identical` strategies while Phase 2
//! handles `SectionMerge` and `DeepMerge` directly.

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
