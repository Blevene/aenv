//! `aenv activate [<name>] [--project <path>]`.
//!
//! If `name` is omitted, read the project's `.aenv` pin.

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::project::read_pin;
use aenv_core::Result;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    project_root: &Path,
    namespace_name: Option<&str>,
) -> Result<()> {
    let name = match namespace_name {
        Some(n) => n.to_string(),
        None => read_pin(fs, project_root)?,
    };
    let leaf = NamespaceId::new(name.clone())
        .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("namespace id: {e}")))?;
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let state = activate_namespace(fs, layout, &adapters, project_root, &leaf)?;
    for w in &state.warnings {
        eprintln!("[aenv] warning: {w}");
    }
    println!("Activated '{}' in {}", name, project_root.display());
    for file in &state.managed_files {
        println!("  + {} ({:?})", file.path.display(), file.strategy);
    }
    if !state.backed_up.is_empty() {
        println!("Backed up {} file(s):", state.backed_up.len());
        for backup in &state.backed_up {
            println!(
                "  - {} -> {}",
                backup.original_path.display(),
                backup.backup_path.display()
            );
        }
    }
    Ok(())
}
