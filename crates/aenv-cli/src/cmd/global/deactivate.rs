//! `aenv global deactivate` — reverse a global activation.
//!
//! Exit 0 with a no-op message when no global activation is live.

use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, fake_home: &Path) -> Result<()> {
    if !fs.exists(&layout.global_state_path())? {
        println!("no global activation to deactivate");
        return Ok(());
    }
    let active = aenv_core::deactivate::deactivate_namespace_in_scope(
        fs,
        layout,
        fake_home,
        aenv_core::scope::Scope::User,
    )?;
    println!("Deactivated '{active}' globally.");
    Ok(())
}
