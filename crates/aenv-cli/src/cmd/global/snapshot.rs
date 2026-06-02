//! `aenv global snapshot <name>` — capture the current `$HOME` user-scope
//! surface into a new namespace.
//!
//! Dual of `aenv global activate`: instead of writing namespace bytes into
//! `$HOME`, this reads from `$HOME` and writes a namespace recipe that
//! re-activates byte-identically.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    fake_home: &Path,
    name: &str,
    include: &[String],
    shared: bool,
) -> Result<()> {
    let summary = aenv_core::global_snapshot::snapshot_global(
        fs, layout, adapters, fake_home, name, include, shared,
    )?;
    println!(
        "Snapshotted current ~/ user-scope surface into namespace '{name}' ({} file{}, {} director{} captured).",
        summary.files_copied,
        if summary.files_copied == 1 { "" } else { "s" },
        summary.directories_copied,
        if summary.directories_copied == 1 { "y" } else { "ies" },
    );
    for p in &summary.user_files_declared {
        println!("  + {p}");
    }
    Ok(())
}
