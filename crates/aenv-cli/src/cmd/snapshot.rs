//! `aenv snapshot <name>` — capture current project state into a new namespace.
//!
//! Walks each installed adapter's `files = [...]` patterns against the project
//! tree, copies every matching file into a fresh namespace directory, and writes
//! a manifest declaring them. The namespace is self-contained and portable; the
//! project pin is **not** updated (unlike `aenv fork <name>`).
//!
//! Refuses with exit 12 if `<name>` already exists, and with a helpful error
//! if the project has no adapter-managed files at all.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::AenvError;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::Result;
use std::path::Path;

/// Snapshot the current project into a new namespace.
///
/// - `name`         — namespace name to create.
/// - `project_root` — absolute path to the project root.
/// - `extends`      — optional parent namespaces (`--extends` flag, may be empty).
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    name: &str,
    extends: &[String],
) -> Result<()> {
    // Delegate to the shared library function which handles all file-walking,
    // glob expansion, symlink resolution, and manifest writing. It already
    // refuses (ManifestInvalid / exit 12) if the namespace exists.
    aenv_core::namespace::create_namespace_from_project(
        fs,
        layout,
        adapters,
        name,
        project_root,
        extends,
    )?;

    // Count captured files by summing adapter entries in the written manifest.
    // Re-read the manifest to get the accurate per-adapter breakdown.
    let manifest_path = layout.manifest_path(name);
    let manifest_bytes = fs.read(&manifest_path)?;
    let manifest_str = std::str::from_utf8(&manifest_bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("manifest not utf-8: {e}")))?;
    let manifest = aenv_core::manifest::AenvManifest::from_toml(manifest_str)?;

    let total: usize = manifest.adapters.values().map(|a| a.files.len()).sum();

    if total == 0 {
        // The namespace was created but is empty — remove it and report a
        // helpful error rather than leaving an empty namespace behind.
        let ns_dir = layout.namespace_dir(name);
        let _ = fs.remove_dir_all(&ns_dir); // best-effort cleanup
        return Err(AenvError::ManifestInvalid(format!(
            "project at '{}' has no adapter-managed files; nothing to snapshot",
            project_root.display()
        )));
    }

    println!(
        "Snapshotted {total} file{} from '{}' into namespace '{name}'.",
        if total == 1 { "" } else { "s" },
        project_root.display()
    );
    for (adapter_name, entry) in &manifest.adapters {
        let count = entry.files.len();
        println!(
            "  {adapter_name}: {count} file{}",
            if count == 1 { "" } else { "s" }
        );
    }

    Ok(())
}
