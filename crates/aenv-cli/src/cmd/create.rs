//! `aenv create <name>` — scaffold a new namespace.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::adapters_builtin;
use aenv_core::error::AenvError;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::namespace::create_namespace;
use aenv_core::Result;

/// Create a new namespace, installing built-in adapters on first run.
///
/// `adapter_names` seeds `[adapters.<name>]` blocks in the generated manifest
/// AND scaffolds an empty file on disk for every literal (non-glob,
/// non-trailing-slash) entry in each adapter's declared `files`. The same
/// paths are recorded in the manifest's `files = [...]`, so `aenv activate`
/// on a freshly-created namespace immediately materializes a working,
/// editable file tree — no follow-up edit of `aenv.toml` required.
///
/// Each adapter name is validated against `adapters_reg` before any file is
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
    if !adapter_names.is_empty() {
        scaffold_adapter_files(fs, layout, adapters_reg, name, adapter_names)?;
    }
    println!(
        "Created namespace '{}' at {}",
        name,
        layout.namespace_dir(name).display()
    );
    Ok(())
}

/// For each requested adapter, create an empty file on disk for every concrete
/// entry in its `files` declaration and record those paths in the manifest's
/// `[adapters.<name>].files`. Entries that are globs (`*`) or directory
/// markers (trailing `/`) are skipped — the user populates those via
/// `aenv skill new` or by hand.
fn scaffold_adapter_files<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters_reg: &AdapterRegistry,
    namespace: &str,
    adapter_names: &[String],
) -> Result<()> {
    let manifest_path = layout.manifest_path(namespace);
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        AenvError::ManifestInvalid(format!("just-written manifest is not utf-8: {e}"))
    })?;
    let mut manifest = AenvManifest::from_toml(text)?;

    let ns_dir = layout.namespace_dir(namespace);
    for adapter_name in adapter_names {
        let adapter = adapters_reg
            .get(adapter_name)
            .expect("CLI validated adapter name before reaching this branch");
        let entry = manifest
            .adapters
            .get_mut(adapter_name)
            .expect("create_namespace just inserted this adapter block");
        for f in &adapter.files {
            let is_glob = f.contains('*');
            let is_dir_marker = f.ends_with('/');
            if is_glob || is_dir_marker {
                continue;
            }
            fs.write(&ns_dir.join(f), b"")?;
            entry.files.push(f.clone());
        }
    }
    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    Ok(())
}
