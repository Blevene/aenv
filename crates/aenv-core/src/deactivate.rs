//! Deactivation: remove every file aenv materialized, restore backups,
//! delete state.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
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
///
/// Thin wrapper for `Scope::Project`. For user-scope deactivation, call
/// [`deactivate_namespace_in_scope`] directly with a `RegistryLayout`.
pub fn deactivate_namespace<F: Filesystem>(fs: &F, project_root: &Path) -> Result<String> {
    let state_path = project_root.join(".aenv-state/state.json");
    deactivate_with_state_path(fs, &state_path, project_root, crate::scope::Scope::Project)
}

/// Scope-aware deactivation. For [`Scope::Project`](crate::scope::Scope::Project),
/// behaves exactly like [`deactivate_namespace`]: reads
/// `<target_root>/.aenv-state/state.json` and restores backups under
/// `<target_root>/`. For [`Scope::User`](crate::scope::Scope::User), reads
/// `<layout.root()>/global-state.json` and restores backups under
/// `<target_root>/` (which the CLI sets to `$HOME`).
///
/// In both cases the recorded `BackedUpFile::backup_path` is the path
/// written at activate time — project-scope backups live under
/// `<target_root>/.aenv-state/backup/<ts>/` and user-scope backups live
/// under `<aenv_home>/global-stash/<ts>/`. The rename logic does not need
/// to distinguish the two: it operates on absolute paths as recorded.
pub fn deactivate_namespace_in_scope<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    target_root: &Path,
    scope: crate::scope::Scope,
) -> Result<String> {
    let lock = if scope == crate::scope::Scope::User {
        Some(crate::global_lock::acquire_global_lock(
            &layout.global_lock_path(),
        )?)
    } else {
        None
    };
    let result = deactivate_in_scope_inner(fs, layout, target_root, scope);
    if let Some(handle) = lock {
        let _ = crate::global_lock::release_global_lock(handle);
    }
    result
}

/// Inner deactivation routine — no lock acquisition. Callers that already
/// hold the global lock (e.g. `swap_or_activate_user_inner`) call this
/// directly to avoid double-locking.
pub(crate) fn deactivate_in_scope_inner<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    target_root: &Path,
    scope: crate::scope::Scope,
) -> Result<String> {
    let state_path = match scope {
        crate::scope::Scope::Project => target_root.join(".aenv-state/state.json"),
        crate::scope::Scope::User => layout.global_state_path(),
    };
    deactivate_with_state_path(fs, &state_path, target_root, scope)
}

/// Core deactivation routine: same logic for both scopes, parameterized by
/// where the state file lives, where managed files are anchored, and which
/// scope-specific cleanup to do at the end.
fn deactivate_with_state_path<F: Filesystem>(
    fs: &F,
    state_path: &Path,
    target_root: &Path,
    scope: crate::scope::Scope,
) -> Result<String> {
    if !fs.exists(state_path)? {
        return Err(AenvError::ActivationConflict(format!(
            "no active namespace in {}",
            target_root.display()
        )));
    }
    let bytes = fs.read(state_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("state.json: not utf-8: {e}")))?;
    let state = ActivationState::from_json(text)?;
    let active_namespace = state.active_namespace.clone();

    // Remove materialized files first.
    for file in &state.managed_files {
        let project_path = target_root.join(&file.path);
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
        let original = target_root.join(&backup.original_path);
        let backup_path = target_root.join(&backup.backup_path);
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
    prune_empty_parents(target_root, &state.managed_files);

    // Remove the state file last — its presence is the signal that there's
    // anything to deactivate.
    fs.remove_file(state_path)?;

    // Best-effort: for project scope only, if `.aenv-state/` is now empty
    // remove it too. Ignore the error — a non-empty directory (user files,
    // stale backup) is fine to leave. For user scope, `<aenv_home>` holds
    // the registry (adapters, envs) so we never touch it; orphan stash
    // directories under `global-stash/<ts>/` are surfaced by
    // `aenv global doctor` (Task 20) which offers `--prune` for cleanup.
    if scope == crate::scope::Scope::Project {
        let state_dir = target_root.join(".aenv-state");
        let _ = std::fs::remove_dir(&state_dir);
    }

    Ok(active_namespace)
}

/// Remove every empty parent directory of every removed managed file, up to
/// but not including `floor`. Best-effort: a non-empty directory (because
/// the user has their own files there, or another managed file from the
/// same skill still occupies the path) is left alone — that's exactly what
/// `std::fs::remove_dir` does already.
///
/// Deepest-first ordering matters: pruning `<skill>/references/` after
/// `<skill>/references/api.md` is removed unblocks pruning `<skill>/` next.
fn prune_empty_parents(floor: &Path, files: &[crate::state::ManagedFile]) {
    use std::collections::BTreeSet;
    let mut parents: BTreeSet<std::path::PathBuf> = BTreeSet::new();
    for file in files {
        let full = floor.join(&file.path);
        let mut cur = full.parent();
        while let Some(dir) = cur {
            if dir == floor || !dir.starts_with(floor) {
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
