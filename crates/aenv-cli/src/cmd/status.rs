//! `aenv status [--project <path>]`.

use aenv_core::fs::Filesystem;
use aenv_core::state::{ActivationState, MaterializeStrategy};
use aenv_core::Result;
use std::path::Path;

fn strategy_label(s: MaterializeStrategy) -> &'static str {
    match s {
        MaterializeStrategy::Symlink => "symlink",
        MaterializeStrategy::Copy => "copy",
        MaterializeStrategy::Identical => "identical",
        MaterializeStrategy::Merged => "merged",
    }
}

pub fn run<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let state_path = project_root.join(".aenv/state.json");
    if !fs.exists(&state_path)? {
        println!("No active namespace in {}", project_root.display());
        return Ok(());
    }
    let bytes = fs.read(&state_path)?;
    let text = String::from_utf8(bytes)
        .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("state.json: {e}")))?;
    let state = ActivationState::from_json(&text)?;
    println!("Active namespace: {}", state.active_namespace);
    println!("Project root: {}", state.project_root.display());
    println!("Managed files ({}):", state.managed_files.len());
    for file in &state.managed_files {
        println!(
            "  {} ({})",
            file.path.display(),
            strategy_label(file.strategy)
        );
    }
    if !state.backed_up.is_empty() {
        println!("Backed up ({}):", state.backed_up.len());
        for backup in &state.backed_up {
            println!(
                "  {} -> {}",
                backup.original_path.display(),
                backup.backup_path.display()
            );
        }
    }
    Ok(())
}
