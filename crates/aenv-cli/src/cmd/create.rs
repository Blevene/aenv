//! `aenv create <name>` — scaffold a new namespace.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::adapters_builtin;
use aenv_core::error::AenvError;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::create_namespace;
use aenv_core::Result;

/// Create a new namespace, installing built-in adapters on first run.
///
/// `adapter_names` seeds empty `[adapters.<name>]` blocks in the generated
/// manifest. Each name is validated against `adapters_reg` before any file is
/// written; an unknown name returns `AdapterMissing` (exit 11).
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters_reg: &AdapterRegistry,
    name: &str,
    extends: &[String],
    adapter_names: &[String],
) -> Result<()> {
    // Validate all adapter names before touching the filesystem.
    for adapter_name in adapter_names {
        if adapters_reg.get(adapter_name).is_none() {
            return Err(AenvError::AdapterMissing(adapter_name.clone()));
        }
    }
    adapters_builtin::install_builtins(fs, &layout.adapters_dir())?;
    create_namespace(fs, layout, name, extends, adapter_names)?;
    println!(
        "Created namespace '{}' at {}",
        name,
        layout.namespace_dir(name).display()
    );
    Ok(())
}
