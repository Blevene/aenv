//! `aenv skill remove <name> --ns <ns>` — drop a skill from a namespace.
//!
//! Removes the `[[skills]]` entry from the manifest and, for authored
//! skills only, deletes the on-disk directory under
//! `<ns>/.claude/skills/<name>/`. Imported skills leave the
//! `~/.aenv/cache/skills/<hash>/<ref>/` clone in place — it's cheap, and
//! the user can run `aenv cache prune` to reclaim space across all
//! namespaces.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::SkillMode;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    namespace: &str,
    skill_name: &str,
) -> Result<()> {
    let manifest_path = layout.manifest_path(namespace);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(namespace.to_string()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("manifest not utf-8: {e}")))?;
    let mut manifest = AenvManifest::from_toml(text)?;

    let position = manifest.skills.iter().position(|s| s.name == skill_name);
    let removed = match position {
        Some(idx) => manifest.skills.remove(idx),
        None => {
            return Err(AenvError::ManifestInvalid(format!(
                "namespace '{namespace}' has no skill named '{skill_name}'"
            )));
        }
    };

    // Authored skills own their files in the namespace tree; delete those.
    // Imported skills cache fetched content under ~/.aenv/cache/skills/ —
    // we leave the cache alone here so it can serve other namespaces using
    // the same source+ref; `aenv cache prune` reclaims it.
    if matches!(removed.mode, SkillMode::Authored) {
        // Determine the skills_dir from the adapter declaration if present,
        // else fall back to the convention `.claude/skills`.
        let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
        let skills_dir = removed
            .adapter
            .as_deref()
            .and_then(|n| adapters.get(n))
            .and_then(|a| a.skills_dir.clone())
            .unwrap_or_else(|| ".claude/skills".to_string());
        let skill_dir = layout
            .namespace_dir(namespace)
            .join(&skills_dir)
            .join(skill_name);
        if fs.exists(&skill_dir)? {
            fs.remove_dir_all(&skill_dir)?;
        }
    }

    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    println!(
        "Removed skill '{skill_name}' from namespace '{namespace}' ({} skill).",
        match removed.mode {
            SkillMode::Authored => "authored",
            SkillMode::Imported => "imported",
        }
    );
    if matches!(removed.mode, SkillMode::Imported) {
        println!("  - cache left in place; run `aenv cache prune` to reclaim space.");
    }
    println!("  - re-activate any project using '{namespace}' to drop the materialized symlink.");
    Ok(())
}
