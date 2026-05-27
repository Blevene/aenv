//! `aenv deactivate [--project <path>] [--prune]`.

use aenv_core::deactivate::deactivate_namespace;
use aenv_core::fs::Filesystem;
use aenv_core::Result;
use std::path::Path;

pub fn run<F: Filesystem>(fs: &F, project_root: &Path, prune: bool) -> Result<()> {
    let name = deactivate_namespace(fs, project_root)?;
    println!(
        "Deactivated namespace '{name}' in {}",
        project_root.display()
    );

    if prune {
        let backup_root = project_root.join(".aenv-state/backup");
        let mut removed = 0usize;
        if backup_root.exists() {
            for entry in std::fs::read_dir(&backup_root).map_err(aenv_core::AenvError::Io)? {
                let path = entry.map_err(aenv_core::AenvError::Io)?.path();
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(&path);
                    removed += 1;
                }
            }
            // If the backup dir is now empty, remove it too. Then walk up to
            // collapse `.aenv-state/` if nothing else remains there.
            let _ = std::fs::remove_dir(&backup_root);
            let _ = std::fs::remove_dir(project_root.join(".aenv-state"));
        }
        if removed > 0 {
            println!(
                "Pruned {removed} backup director{}.",
                if removed == 1 { "y" } else { "ies" }
            );
        }
    }
    Ok(())
}
