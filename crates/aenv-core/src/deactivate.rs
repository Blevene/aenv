//! Deactivation: remove every file aenv materialized, restore backups,
//! delete state.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::state::{ActivationState, MaterializeStrategy};
use std::path::Path;

/// Deactivate the namespace currently active in `project_root`.
///
/// Reads `.aenv-state/state.json` to know what to undo. Files with strategy
/// `Symlink` or `Copy` are removed; the corresponding backed-up original
/// (if any) is renamed back into place. Files with strategy `Identical`
/// are left alone — they were the user's to begin with. After a
/// successful deactivation, `.aenv-state/state.json` is removed.
///
/// Missing state.json -> `ActivationConflict` (exit 13). A missing pin
/// file is a distinct condition (`ProjectNotPinned`, exit 20) — a user
/// can be pinned but not activated.
pub fn deactivate_namespace<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        return Err(AenvError::ActivationConflict(format!(
            "no active namespace in {}",
            project_root.display()
        )));
    }
    let bytes = fs.read(&state_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("state.json: not utf-8: {e}")))?;
    let state = ActivationState::from_json(text)?;

    // Remove materialized files first.
    for file in &state.managed_files {
        let project_path = project_root.join(&file.path);
        match file.strategy {
            MaterializeStrategy::Symlink | MaterializeStrategy::Copy => {
                // Best-effort: user may have removed it already.
                let _ = fs.remove_file(&project_path);
            }
            MaterializeStrategy::Identical => {
                // Leave in place: it's the user's file.
            }
            MaterializeStrategy::Merged
            | MaterializeStrategy::SectionMerge
            | MaterializeStrategy::DeepMerge(_) => {
                // Merged output is a regular file written by aenv; remove it.
                let _ = fs.remove_file(&project_path);
            }
        }
    }

    // Restore backups (rename backup -> original).
    for backup in &state.backed_up {
        let original = project_root.join(&backup.original_path);
        let backup_path = project_root.join(&backup.backup_path);
        // If something now occupies the original path, remove it first.
        if fs.exists(&original)? {
            let _ = fs.remove_file(&original);
        }
        fs.rename(&backup_path, &original)?;
    }

    // Remove the state file last — its presence is the signal that there's
    // anything to deactivate.
    fs.remove_file(&state_path)?;
    Ok(())
}
