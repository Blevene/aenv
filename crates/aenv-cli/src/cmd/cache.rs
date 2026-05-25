//! `aenv cache <action>` — manage `~/.aenv/cache/`.
//!
//! Today the only action is `prune`, which walks every namespace's
//! `[[skills]]` entries, collects the `(source-hash, ref)` cache dirs
//! that are in use, and deletes anything else under
//! `~/.aenv/cache/skills/`. Local-source and registry-source skills
//! never have a cache entry, so they're ignored.

use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::namespace::list_namespaces;
use aenv_core::skills::cache::source_hash;
use aenv_core::skills::source::SourceKind;
use aenv_core::skills::SkillMode;
use std::collections::HashSet;

pub fn run_prune<F: Filesystem>(fs: &F, layout: &RegistryLayout) -> Result<()> {
    let cache_root = layout.skills_cache_dir();
    if !fs.exists(&cache_root)? {
        println!("Cache empty (nothing to prune).");
        return Ok(());
    }

    // Collect (source-hash, ref-label) pairs from every namespace's [[skills]].
    // The ref-label is whatever was recorded as `ref =` in the manifest, or
    // the literal "head" when omitted (matching skill_cache_path's default).
    let mut in_use: HashSet<(String, String)> = HashSet::new();
    for ns in list_namespaces(fs, layout)? {
        let manifest_path = layout.manifest_path(&ns);
        let bytes = match fs.read(&manifest_path) {
            Ok(b) => b,
            Err(_) => continue, // manifest unreadable; skip
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let manifest = match AenvManifest::from_toml(text) {
            Ok(m) => m,
            Err(_) => continue,
        };
        for skill in &manifest.skills {
            if !matches!(skill.mode, SkillMode::Imported) {
                continue;
            }
            let source = match skill.source.as_deref() {
                Some(s) => s,
                None => continue,
            };
            let (url, frag_ref) = match SourceKind::parse(source) {
                Ok(SourceKind::Git { url, ref_spec }) => (url, ref_spec),
                Ok(_) => continue, // local + registry leave no cache
                Err(_) => continue,
            };
            let ref_label = skill
                .ref_
                .clone()
                .or(frag_ref)
                .unwrap_or_else(|| "head".to_string());
            in_use.insert((source_hash(&url), ref_label));
        }
    }

    // Walk cache_root/<source-hash>/<ref-label>/, remove anything not in_use.
    let mut removed = 0usize;
    let mut empty_parents = Vec::new();
    for hash_dir in fs.list_dir(&cache_root)? {
        let hash = match hash_dir.file_name().and_then(|s| s.to_str()) {
            Some(h) => h.to_string(),
            None => continue,
        };
        let mut hash_dir_now_empty = true;
        for ref_dir in fs.list_dir(&hash_dir)? {
            let ref_label = match ref_dir.file_name().and_then(|s| s.to_str()) {
                Some(r) => r.to_string(),
                None => {
                    hash_dir_now_empty = false;
                    continue;
                }
            };
            if in_use.contains(&(hash.clone(), ref_label.clone())) {
                hash_dir_now_empty = false;
                continue;
            }
            fs.remove_dir_all(&ref_dir)?;
            removed += 1;
        }
        if hash_dir_now_empty {
            empty_parents.push(hash_dir);
        }
    }
    for dir in empty_parents {
        // Best-effort: remove the now-empty source-hash directory.
        let _ = std::fs::remove_dir(&dir);
    }

    println!(
        "Pruned {removed} stale cache director{} from ~/.aenv/cache/skills/.",
        if removed == 1 { "y" } else { "ies" }
    );
    println!(
        "  {} (source, ref) pair{} still referenced.",
        in_use.len(),
        if in_use.len() == 1 { "" } else { "s" }
    );
    Ok(())
}
