//! `aenv delete <name>` — remove a namespace from the registry.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::delete_namespace;
use aenv_core::Result;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, name: &str) -> Result<()> {
    // PRD R-4 expects checking that the namespace isn't currently active in
    // any tracked project. A project-tracking registry doesn't exist yet, so
    // we can't verify this. Warn loudly before destroying the namespace; a
    // proper safety check (turning this into a hard refusal) arrives with
    // Phase 6's project-tracking registry.
    eprintln!(
        "warning: aenv cannot yet verify namespace '{name}' isn't actively pinned \
         in a project. Delete is irreversible — if any project is using it, \
         run `aenv unpin --project <path>` there first to avoid an orphan state file."
    );
    delete_namespace(fs, layout, name)?;
    println!("Deleted namespace '{name}'");
    Ok(())
}
