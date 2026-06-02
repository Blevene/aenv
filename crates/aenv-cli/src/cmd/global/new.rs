//! `aenv global new <name>` — scaffold an empty, editable user-scope namespace.
//!
//! The from-scratch counterpart to `aenv global snapshot` (capture `$HOME`)
//! and `aenv global import` (capture an external tree). It seeds a minimal,
//! hand-authorable namespace — the adapter's instructions file under `user/`
//! plus a pre-wired manifest — so authoring your own profile isn't a manual
//! `mkdir`/edit-`aenv.toml` ritual.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::adapters_builtin;
use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    name: &str,
    adapter: &str,
    shared: bool,
) -> Result<()> {
    // Ensure built-in adapters are installed so a fresh registry can scaffold
    // against the default `claude-code` adapter without a prior `aenv create`,
    // then load the registry that includes them.
    adapters_builtin::install_builtins(fs, &layout.adapters_dir())?;
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;

    let summary = aenv_core::global_snapshot::scaffold_global_namespace(
        fs, layout, &adapters, name, adapter, shared,
    )?;

    println!(
        "Created user-scope namespace '{name}' at {}",
        layout.namespace_dir(name).display()
    );
    if let Some(seed) = &summary.seeded_instructions {
        println!("  + user/{seed}  (edit this, then run: aenv global use {name})");
    } else {
        println!(
            "  (no adapter user-files to seed; add files under {}/user/ and declare them in aenv.toml)",
            layout.namespace_dir(name).display()
        );
    }
    Ok(())
}
