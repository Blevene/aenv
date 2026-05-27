//! `aenv global use <ns>` — activate a namespace globally.

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
    let count = state.managed_files.len();
    println!(
        "Activated '{}' globally — {count} file{} materialized under {}.",
        state.active_namespace,
        if count == 1 { "" } else { "s" },
        fake_home.display()
    );
    println!("Note: running harness sessions retain their previous config until restart.");
    Ok(())
}
