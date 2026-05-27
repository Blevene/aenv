//! Rename atomicity probe (engineering §7).
//!
//! `std::fs::rename` is atomic on Unix *only when source and destination
//! are on the same filesystem*. If a project's `.aenv/` directory ends up
//! on a different mount (e.g. symlinked elsewhere), rename silently
//! degrades to copy+delete and we lose the atomicity guarantee R-45
//! depends on.
//!
//! The probe writes two tiny files inside `.aenv/`, renames one to the
//! other, and removes the survivor. If the rename succeeds the assumption
//! holds; failure surfaces as `ActivationConflict` (exit 13).

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use std::path::Path;

/// Probe rename atomicity at an arbitrary directory. The directory will be
/// created if needed; probe files are cleaned up on success.
pub fn probe_rename_atomicity_at<F: Filesystem>(fs: &F, dir: &Path) -> Result<()> {
    fs.create_dir_all(dir)?;
    let a = dir.join(".probe.a");
    let b = dir.join(".probe.b");
    // Cleanup any stale probe files from a previous interrupted run.
    let _ = fs.remove_file(&a);
    let _ = fs.remove_file(&b);
    fs.write(&a, b"probe").map_err(|e| {
        AenvError::ActivationConflict(format!("atomicity probe: write failed: {e}"))
    })?;
    fs.rename(&a, &b).map_err(|e| {
        let _ = fs.remove_file(&a);
        AenvError::ActivationConflict(format!("atomicity probe: rename failed: {e}"))
    })?;
    fs.remove_file(&b).map_err(|e| {
        AenvError::ActivationConflict(format!("atomicity probe: cleanup failed: {e}"))
    })?;
    Ok(())
}

/// Backward-compatible wrapper: probes `<project_root>/.aenv-state/`.
pub fn probe_rename_atomicity<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    probe_rename_atomicity_at(fs, &project_root.join(".aenv-state"))
}
