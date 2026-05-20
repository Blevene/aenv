//! `aenv list` — print every namespace in the registry.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::list_namespaces;
use aenv_core::Result;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout) -> Result<()> {
    let names = list_namespaces(fs, layout)?;
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
