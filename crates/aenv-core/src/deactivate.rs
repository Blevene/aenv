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
/// Returns the name of the namespace that was deactivated, so callers can
/// surface it in user-facing messages without re-reading state.json.
///
/// Missing state.json -> `ActivationConflict` (exit 13). A missing pin
/// file is a distinct condition (`ProjectNotPinned`, exit 20) — a user
/// can be pinned but not activated.
pub fn deactivate_namespace<F: Filesystem>(fs: &F, project_root: &Path) -> Result<String> {
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
    let active_namespace = state.active_namespace.clone();

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

    // Best-effort: prune empty parent dirs left behind by removed files
    // (e.g. `.claude/skills/<skill>/references/` after a skill's symlinks
    // go away). `std::fs::remove_dir` is a no-op on non-empty directories,
    // so adjacent user files are never touched. Done BEFORE state removal
    // so an interrupted run leaves the state pointer intact for retry.
    prune_empty_parents(project_root, &state.managed_files);

    // Remove the state file last — its presence is the signal that there's
    // anything to deactivate.
    fs.remove_file(&state_path)?;

    // Best-effort: if the state directory is now empty, remove it too.
    // Ignore the error — a non-empty directory (user files, stale backup) is fine to leave.
    let state_dir = project_root.join(".aenv-state");
    let _ = std::fs::remove_dir(&state_dir);

    Ok(active_namespace)
}

/// Remove every empty parent directory of every removed managed file, up to
/// but not including `project_root`. Best-effort: a non-empty directory
/// (because the user has their own files there, or another managed file
/// from the same skill still occupies the path) is left alone — that's
/// exactly what `std::fs::remove_dir` does already.
///
/// Deepest-first ordering matters: pruning `<skill>/references/` after
/// `<skill>/references/api.md` is removed unblocks pruning `<skill>/` next.
fn prune_empty_parents(project_root: &Path, files: &[crate::state::ManagedFile]) {
    use std::collections::BTreeSet;
    let mut parents: BTreeSet<std::path::PathBuf> = BTreeSet::new();
    for file in files {
        let full = project_root.join(&file.path);
        let mut cur = full.parent();
        while let Some(dir) = cur {
            if dir == project_root || !dir.starts_with(project_root) {
                break;
            }
            parents.insert(dir.to_path_buf());
            cur = dir.parent();
        }
    }
    let mut sorted: Vec<_> = parents.into_iter().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for dir in sorted {
        let _ = std::fs::remove_dir(&dir);
    }
}
