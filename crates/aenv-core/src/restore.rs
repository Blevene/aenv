//! Restore the most recent backup set.
//!
//! R-62: `aenv restore` restores the latest backup even when no namespace
//! is active. Useful when a user manually removed a symlink or wants to
//! recover an original after a forced deactivation.
//!
//! Restore semantics are **copy**, not move — the backup directory is left
//! intact so the same backup set can be restored repeatedly. Note that
//! `aenv deactivate` uses *rename* (move) semantics on the backup,
//! consuming it; if the user deactivates first, the backup is gone and
//! restore will report no backups available. Restore is the recovery path
//! for "deactivation never happened" or "the backup is still there because
//! it wasn't the most recent activation's."

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use std::path::Path;

/// Restore the most recent backup set under `<project>/.aenv-state/backup/`.
/// Latest is determined by lex-order on the timestamp directory name
/// (matching how `backup_timestamp()` formats it).
pub fn restore_latest_backup<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let backup_root = project_root.join(".aenv-state/backup");
    if !fs.exists(&backup_root)? {
        return Err(AenvError::ActivationConflict(
            "no backups found under .aenv-state/backup/".to_string(),
        ));
    }
    let mut sets = fs.list_dir(&backup_root)?;
    sets.sort();
    let latest = sets.last().ok_or_else(|| {
        AenvError::ActivationConflict("no backup sets in .aenv-state/backup/".to_string())
    })?;

    // Walk the backup set, restoring every file with the correct
    // project-relative path.
    let prefix = latest.clone();
    let mut to_visit = vec![prefix.clone()];
    while let Some(dir) = to_visit.pop() {
        for entry in fs.list_dir(&dir)? {
            let meta = fs.symlink_metadata(&entry)?;
            if matches!(meta.kind, crate::fs::FileKind::Directory) {
                to_visit.push(entry);
                continue;
            }
            // Compute the project-relative path by stripping the timestamp prefix.
            let rel = entry
                .strip_prefix(&prefix)
                .map_err(|e| AenvError::ActivationConflict(format!("bad backup path: {e}")))?
                .to_path_buf();
            let target = project_root.join(&rel);
            // If the project path currently has something at it, drop it.
            if fs.exists(&target)? {
                let _ = fs.remove_file(&target);
            }
            // Copy bytes (rename across the backup dir would change the
            // backup set; copy keeps the backup intact for re-restore).
            let bytes = fs.read(&entry)?;
            fs.write(&target, &bytes)?;
        }
    }
    Ok(())
}
