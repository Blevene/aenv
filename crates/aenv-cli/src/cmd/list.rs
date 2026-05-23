//! `aenv list` — print every namespace in the registry.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::Result;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, json: bool) -> Result<()> {
    let names = aenv_core::namespace::list_namespaces(fs, layout)?;

    if json {
        let adapters =
            aenv_core::adapter::AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
        let entries: Vec<aenv_core::json::ListEntry> = names
            .iter()
            .map(|n| aenv_core::json::ListEntry::build(fs, layout, &adapters, n))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&entries)
                .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("json: {e}")))?
        );
        return Ok(());
    }

    if names.is_empty() {
        println!("No namespaces in registry at {}", layout.root().display());
        return Ok(());
    }
    println!("NAME");
    for name in names {
        println!("{name}");
    }
    Ok(())
}
