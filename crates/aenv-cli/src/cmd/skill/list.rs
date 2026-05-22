//! `aenv skill list [--ns <ns>]` — text-table output of every skill.

use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::SkillMode;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, ns_filter: Option<&str>) -> Result<()> {
    let envs_dir = layout.namespaces_dir();
    let namespaces: Vec<String> = if !fs.exists(&envs_dir)? {
        Vec::new()
    } else {
        let mut names: Vec<String> = fs
            .list_dir(&envs_dir)?
            .into_iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
            .filter(|name| ns_filter.is_none_or(|f| f == name))
            .collect();
        names.sort();
        names
    };

    println!(
        "{:<20}  {:<30}  {:<10}  {:<60}  PIN",
        "ENV", "SKILL", "MODE", "SOURCE"
    );
    for ns in &namespaces {
        let manifest_path = layout.manifest_path(ns);
        if !fs.exists(&manifest_path)? {
            continue;
        }
        let bytes = fs.read(&manifest_path)?;
        let text = std::str::from_utf8(&bytes).unwrap_or("");
        let manifest = match AenvManifest::from_toml(text) {
            Ok(m) => m,
            Err(_) => continue,
        };
        for s in &manifest.skills {
            let mode = match s.mode {
                SkillMode::Authored => "authored",
                SkillMode::Imported => "imported",
            };
            let source = s.source.as_deref().unwrap_or("-");
            // Per functional spec §5.11, unpinned imported skills render
            // as "(head)" — they resolve to head on each activation.
            // Authored skills render as "-" since pinning doesn't apply.
            let pin = match (s.mode, s.ref_.as_deref()) {
                (_, Some(r)) => r.to_string(),
                (SkillMode::Imported, None) => "(head)".to_string(),
                (SkillMode::Authored, None) => "-".to_string(),
            };
            println!(
                "{:<20}  {:<30}  {:<10}  {:<60}  {}",
                ns, s.name, mode, source, pin
            );
        }
    }
    Ok(())
}
