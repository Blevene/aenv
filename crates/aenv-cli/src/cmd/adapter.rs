//! `aenv adapter add <path>` / `aenv adapter list`.

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::{AenvError, Result};
use std::path::Path;

/// Validate that an adapter name is filesystem-safe: non-empty, no path
/// separators, no parent-directory traversal, no leading dot. Rejecting
/// these closes a path-traversal hole — a malicious `name = "../../etc/passwd"`
/// in an adapter TOML would otherwise let `run_add` write outside
/// `adapters_dir`.
fn validate_adapter_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(AenvError::ManifestInvalid(
            "adapter name must not be empty".to_string(),
        ));
    }
    if name.starts_with('.') {
        return Err(AenvError::ManifestInvalid(format!(
            "adapter name must not start with '.': {name:?}"
        )));
    }
    for ch in name.chars() {
        if ch == '/' || ch == '\\' || ch == '\0' {
            return Err(AenvError::ManifestInvalid(format!(
                "adapter name contains illegal character {ch:?}: {name:?}"
            )));
        }
    }
    if name == ".." || name.contains("..") {
        return Err(AenvError::ManifestInvalid(format!(
            "adapter name must not contain '..': {name:?}"
        )));
    }
    Ok(())
}

pub fn run_add<F: Filesystem>(fs: &F, layout: &RegistryLayout, source: &Path) -> Result<()> {
    let bytes = fs.read(source)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", source.display())))?;
    let adapter = Adapter::from_toml(text)?;
    validate_adapter_name(&adapter.name)?;
    fs.create_dir_all(&layout.adapters_dir())?;
    let target = layout.adapters_dir().join(format!("{}.toml", adapter.name));
    fs.write(&target, text.as_bytes())?;
    println!(
        "Installed adapter '{}' at {}",
        adapter.name,
        target.display()
    );
    Ok(())
}

pub fn run_list<F: Filesystem>(fs: &F, layout: &RegistryLayout) -> Result<()> {
    let reg = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    if reg.is_empty() {
        println!(
            "No adapters installed at {}",
            layout.adapters_dir().display()
        );
        return Ok(());
    }
    println!("ADAPTER         FILES");
    for (name, adapter) in reg.iter() {
        println!("{:<15} {}", name, adapter.files.join(", "));
    }
    Ok(())
}
