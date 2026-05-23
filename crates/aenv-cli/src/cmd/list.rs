//! `aenv list` — print every namespace in the registry.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
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

    // Text mode: three columns — NAME, EXTENDS, ADAPTERS (R-3).
    println!("{:<22} {:<30} ADAPTERS", "NAME", "EXTENDS");
    for name in &names {
        let manifest = fs
            .read(&layout.manifest_path(name))
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| AenvManifest::from_toml(&s).ok());
        let (extends_str, adapters_str) = match manifest {
            None => ("<error>".to_string(), "<error>".to_string()),
            Some(m) => {
                let ext = if m.extends.is_empty() {
                    "-".to_string()
                } else {
                    m.extends.join(", ")
                };
                let adp = if m.adapters.is_empty() {
                    "-".to_string()
                } else {
                    m.adapters.keys().cloned().collect::<Vec<_>>().join(", ")
                };
                (ext, adp)
            }
        };
        println!("{name:<22} {extends_str:<30} {adapters_str}");
    }
    Ok(())
}
