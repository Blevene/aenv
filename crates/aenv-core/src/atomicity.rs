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

/// Run the probe. Creates `<project>/.aenv/` if it doesn't exist. Leaves
/// no probe files behind on success.
pub fn probe_rename_atomicity<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let aenv_dir = project_root.join(".aenv");
    fs.create_dir_all(&aenv_dir)?;

    let a = aenv_dir.join(".probe.a");
    let b = aenv_dir.join(".probe.b");

    // Cleanup any stale probe files from a previous interrupted run.
    let _ = fs.remove_file(&a);
    let _ = fs.remove_file(&b);

    fs.write(&a, b"probe").map_err(|e| {
        AenvError::ActivationConflict(format!("atomicity probe: write failed: {e}"))
    })?;
    fs.rename(&a, &b).map_err(|e| {
        // Clean up the source before bailing.
        let _ = fs.remove_file(&a);
        AenvError::ActivationConflict(format!("atomicity probe: rename failed: {e}"))
    })?;
    fs.remove_file(&b).map_err(|e| {
        AenvError::ActivationConflict(format!("atomicity probe: cleanup failed: {e}"))
    })?;

    Ok(())
}
