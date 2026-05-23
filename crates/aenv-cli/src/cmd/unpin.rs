//! `aenv unpin [--project <path>]` — remove the .aenv pin from a project.
//!
//! If a namespace is currently active in the project, runs the deactivate
//! flow first so the user gets a single-command disengagement.

use aenv_core::deactivate::deactivate_namespace;
use aenv_core::fs::Filesystem;
use aenv_core::project::read_pin;
use aenv_core::Result;
use std::path::Path;

pub fn run<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let pin_path = project_root.join(".aenv");
    if !fs.exists(&pin_path)? {
        println!("No namespace pinned in {}.", project_root.display());
        return Ok(());
    }

    // Capture the pin contents so we can report which namespace was unpinned.
    let pin_name = read_pin(fs, project_root).unwrap_or_else(|_| "(unknown)".to_string());

    // Auto-deactivate if active.
    let state_path = project_root.join(".aenv-state/state.json");
    if fs.exists(&state_path)? {
        deactivate_namespace(fs, project_root)?;
        println!("Deactivated namespace in {}.", project_root.display());
    }

    // Remove the pin file.
    fs.remove_file(&pin_path)?;
    println!("Unpinned {} (was '{}').", project_root.display(), pin_name);
    Ok(())
}
