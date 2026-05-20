//! `aenv create <name>` — scaffold a new namespace.

use aenv_core::adapters_builtin;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::create_namespace;
use aenv_core::Result;

/// Create a new namespace, installing built-in adapters on first run.
pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, name: &str) -> Result<()> {
    adapters_builtin::install_builtins(fs, &layout.adapters_dir())?;
    create_namespace(fs, layout, name)?;
    println!(
        "Created namespace '{}' at {}",
        name,
        layout.namespace_dir(name).display()
    );
    Ok(())
}
