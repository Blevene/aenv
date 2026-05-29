//! `aenv global deactivate` — reverse a global activation.
//!
//! Exit 0 with a no-op message when no global activation is live.
//!
//! `force = true` skips the namespace's `on_deactivate` lifecycle hook —
//! useful when the hook itself is broken (e.g. a missing interpreter).
//! File restoration proceeds either way.
//!
//! Orphan stash cleanup lives on `aenv global doctor --fix`, not here:
//! deactivation does exactly one thing.

use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    fake_home: &Path,
    force: bool,
) -> Result<()> {
    if !fs.exists(&layout.global_state_path())? {
        println!("no global activation to deactivate");
        return Ok(());
    }
    let active = aenv_core::deactivate::deactivate_namespace_in_scope_with_force(
        fs,
        layout,
        fake_home,
        aenv_core::scope::Scope::User,
        force,
    )?;
    if force {
        println!(
            "Deactivated namespace '{active}' globally in {}. (--force: skipped on_deactivate.)",
            fake_home.display()
        );
    } else {
        println!(
            "Deactivated namespace '{active}' globally in {}",
            fake_home.display()
        );
    }
    Ok(())
}
