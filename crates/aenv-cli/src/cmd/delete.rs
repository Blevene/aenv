//! `aenv delete <name>` — remove a namespace from the registry.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::delete_namespace;
use aenv_core::Result;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, name: &str) -> Result<()> {
    // PRD R-4 expects checking that the namespace isn't currently active in
    // any tracked project. Phase 1 lacks a project-tracking registry, so we
    // can't verify this. Warn loudly before destroying the namespace; a
    // proper safety check arrives with the tracking work later (likely
    // Phase 6, when the shell hook gives us a natural place to maintain a
    // registry of activated projects).
    eprintln!(
        "warning: cannot verify namespace '{name}' is unused; \
         Phase 1 lacks project-tracking. Delete is irreversible."
    );
    delete_namespace(fs, layout, name)?;
    println!("Deleted namespace '{name}'");
    Ok(())
}
