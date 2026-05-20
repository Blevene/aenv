//! `aenv use <name>` — write `.aenv` pin at the project root.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::project::write_pin;
use aenv_core::{AenvError, Result};
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    project_root: &Path,
    name: &str,
) -> Result<()> {
    // Validate the namespace exists before writing the pin — otherwise the
    // user gets a confusing error later from `aenv activate` instead of
    // immediate feedback.
    if !fs.exists(&layout.manifest_path(name))? {
        return Err(AenvError::NamespaceNotFound(name.to_string()));
    }
    write_pin(fs, project_root, name)?;
    println!("Pinned {} to namespace '{}'", project_root.display(), name);
    Ok(())
}
