//! `aenv skill new <name> --ns <ns> [--adapter <a>]` — scaffold authored skill.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::{SkillDecl, SkillMode};

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    namespace: &str,
    skill_name: &str,
    adapter_arg: Option<&str>,
) -> Result<()> {
    let manifest_path = layout.manifest_path(namespace);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(namespace.to_string()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("manifest not utf-8: {e}")))?;
    let mut manifest = AenvManifest::from_toml(text)?;

    // Choose adapter.
    let adapter_name = match adapter_arg {
        Some(a) => a.to_string(),
        None => {
            if manifest.adapters.len() != 1 {
                return Err(AenvError::ManifestInvalid(format!(
                    "namespace '{namespace}' declares {} adapters; use --adapter to disambiguate",
                    manifest.adapters.len()
                )));
            }
            manifest.adapters.keys().next().unwrap().clone()
        }
    };

    let adapter = adapters
        .get(&adapter_name)
        .ok_or_else(|| AenvError::AdapterMissing(adapter_name.clone()))?;
    let skills_dir = adapter.skills_dir.as_deref().ok_or_else(|| {
        AenvError::ManifestInvalid(format!(
            "adapter '{adapter_name}' has no skills_dir; cannot scaffold skills"
        ))
    })?;

    // Reject duplicate name.
    if manifest.skills.iter().any(|s| s.name == skill_name) {
        return Err(AenvError::ManifestInvalid(format!(
            "namespace '{namespace}' already declares a skill '{skill_name}'"
        )));
    }

    // Scaffold SKILL.md.
    let skill_md_path = layout
        .namespace_dir(namespace)
        .join(skills_dir)
        .join(skill_name)
        .join("SKILL.md");
    let body = format!(
        "---\nname: {skill_name}\ndescription: TODO: describe this skill\n---\n\n# {skill_name}\n\nDescribe when the agent should invoke this skill.\n"
    );
    fs.write(&skill_md_path, body.as_bytes())?;

    // Append [[skills]] to the manifest.
    manifest.skills.push(SkillDecl {
        name: skill_name.to_string(),
        mode: SkillMode::Authored,
        adapter: Some(adapter_name),
        source: None,
        ref_: None,
        required: false,
    });
    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    println!("Created authored skill '{skill_name}' in namespace '{namespace}':");
    println!("  - {}", skill_md_path.display());
    println!("  - registered in {}", manifest_path.display());
    Ok(())
}
