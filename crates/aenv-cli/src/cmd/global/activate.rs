//! `aenv global activate <ns>` — activate a namespace globally.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    fake_home: &Path,
    name: &str,
) -> Result<()> {
    let leaf = NamespaceId::new(name).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    let state = aenv_core::activate::swap_or_activate_user(fs, layout, adapters, fake_home, &leaf)?;
    for w in &state.warnings {
        eprintln!("[aenv] warning: {w}");
    }
    println!(
        "Activated '{}' globally in {}",
        state.active_namespace,
        fake_home.display()
    );
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
    println!("Note: running harness sessions retain their previous config until restart.");
    Ok(())
}
