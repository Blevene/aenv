//! `aenv deactivate [--project <path>]`.

use aenv_core::deactivate::deactivate_namespace;
use aenv_core::fs::Filesystem;
use aenv_core::Result;
use std::path::Path;

pub fn run<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    deactivate_namespace(fs, project_root)?;
    println!("Deactivated namespace in {}", project_root.display());
    Ok(())
}
