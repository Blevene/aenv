//! `aenv restore [--project <path>]`.

use aenv_core::fs::Filesystem;
use aenv_core::restore::restore_latest_backup;
use aenv_core::Result;
use std::path::Path;

pub fn run<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    restore_latest_backup(fs, project_root)?;
    println!("Restored most recent backup in {}", project_root.display());
    Ok(())
}
