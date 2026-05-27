//! `aenv global deactivate` — reverse a global activation.
//!
//! Exit 0 with a no-op message when no global activation is live. With
//! `prune = true`, also removes orphan stash directories (subdirs of
//! `<aenv_home>/global-stash/` not referenced by any active state) after
//! the deactivation. Calling `--prune` against a clean home is a no-op.

use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    fake_home: &Path,
    prune: bool,
) -> Result<()> {
    if !fs.exists(&layout.global_state_path())? {
        println!("no global activation to deactivate");
    } else {
        let active = aenv_core::deactivate::deactivate_namespace_in_scope(
            fs,
            layout,
            fake_home,
            aenv_core::scope::Scope::User,
        )?;
        println!("Deactivated '{active}' globally.");
    }

    if prune {
        // Call `list_orphan_stashes` after deactivation: the just-finished
        // activation's stash directory is now orphan and will be pruned in
        // the same call.
        let orphans = aenv_core::state::list_orphan_stashes(layout)?;
        for path in &orphans {
            let _ = std::fs::remove_dir_all(path);
        }
        if !orphans.is_empty() {
            println!(
                "Pruned {} orphan stash{}.",
                orphans.len(),
                if orphans.len() == 1 { "" } else { "es" }
            );
        }
    }

    Ok(())
}
