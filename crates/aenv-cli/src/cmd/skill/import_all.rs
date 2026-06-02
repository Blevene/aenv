//! `aenv skill import-all <source> --ns <ns> [--base <dir>] [--only a,b]
//! [--pin <ref>] [--adapter <a>]` — bulk-import every `<subdir>/SKILL.md`
//! under a base directory of a monorepo skill collection (issue #1).
//!
//! Clones the source ONCE, discovers each `<base>/<subdir>/SKILL.md`, and
//! appends one `[[skills]]` entry per skill in a single manifest write. Each
//! entry is an ordinary `mode = "imported"` skill the per-skill resolver
//! materializes exactly as a hand-typed `aenv skill import --path` would.

use std::collections::BTreeSet;
use std::path::PathBuf;

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::git::git_clone;
use aenv_core::skills::{SkillDecl, SkillMode};

use crate::cmd::global::import::{looks_like_git_url, resolve_local_source, strip_git_prefix};

#[allow(clippy::too_many_arguments)]
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    _adapters: &AdapterRegistry,
    namespace: &str,
    source: &str,
    base: Option<&str>,
    only: Option<&str>,
    pin: Option<&str>,
    adapter_arg: Option<&str>,
    scope: aenv_core::scope::Scope,
) -> Result<()> {
    // 1. Load the manifest + resolve which adapter the skills attach to.
    let manifest_path = layout.manifest_path(namespace);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(namespace.to_string()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("manifest not utf-8: {e}")))?;
    let mut manifest = AenvManifest::from_toml(text)?;
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

    let base = base.unwrap_or("skills");

    // 2. Fetch the source tree once. Git → a throwaway sparse clone of `base`;
    //    local path → walked in place. `_tmp` keeps the tempdir alive.
    let _tmp;
    let (tree_root, resolved_ref): (PathBuf, Option<String>) = if looks_like_git_url(source) {
        let url = strip_git_prefix(source);
        eprintln!(
            "Resolving {source}{}...",
            pin.map(|p| format!(" @ {p}")).unwrap_or_default()
        );
        let tmp = tempfile::tempdir().map_err(AenvError::Io)?;
        let clone_dir = tmp.path().join("clone");
        let sha = git_clone(url, pin, &clone_dir, Some(base))?;
        _tmp = Some(tmp);
        (clone_dir, Some(sha))
    } else {
        if pin.is_some() {
            return Err(AenvError::ManifestInvalid(
                "--pin only applies to git URL sources".into(),
            ));
        }
        _tmp = None;
        (resolve_local_source(source)?, None)
    };

    // 3. Discover `<base>/<subdir>/SKILL.md`. Name = subdir basename.
    let base_dir = tree_root.join(base);
    if !fs.exists(&base_dir)? {
        return Err(AenvError::ManifestInvalid(format!(
            "no '{base}/' directory in the source — check --base (or omit it for the default 'skills')"
        )));
    }
    let mut candidates: Vec<(String, String)> = Vec::new(); // (name, "<base>/<subdir>")
    let mut warnings: Vec<String> = Vec::new();
    let mut entries = fs.list_dir(&base_dir)?;
    entries.sort();
    for entry in entries {
        let skill_md = entry.join("SKILL.md");
        if !fs.exists(&skill_md)? {
            continue;
        }
        let Some(name) = entry
            .file_name()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if !has_name_frontmatter(&fs.read(&skill_md)?) {
            warnings.push(format!(
                "'{name}': SKILL.md has no valid `name:` frontmatter — skipping"
            ));
            continue;
        }
        candidates.push((name.clone(), format!("{base}/{name}")));
    }
    if candidates.is_empty() {
        return Err(AenvError::ManifestInvalid(format!(
            "no `<subdir>/SKILL.md` files under '{base}' — check the path, or omit --base if SKILL.md is at the source root"
        )));
    }

    // 4. `--only` filter: keep the named subset; error before any write if a
    //    requested name isn't present.
    if let Some(only) = only {
        let want: BTreeSet<&str> = only
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let have: BTreeSet<&str> = candidates.iter().map(|(n, _)| n.as_str()).collect();
        let missing: Vec<&str> = want
            .iter()
            .filter(|n| !have.contains(*n))
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(AenvError::ManifestInvalid(format!(
                "--only names not found under '{base}': {}",
                missing.join(", ")
            )));
        }
        candidates.retain(|(n, _)| want.contains(n.as_str()));
    }

    // 5. Skip skills already declared (idempotent); build a decl for the rest.
    let mut imported: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for (name, path) in candidates {
        if manifest.skills.iter().any(|s| s.name == name) {
            skipped.push(name);
            continue;
        }
        manifest.skills.push(SkillDecl {
            name: name.clone(),
            mode: SkillMode::Imported,
            adapter: Some(adapter_name.clone()),
            source: Some(source.to_string()),
            ref_: resolved_ref.clone(),
            path: Some(path),
            required: false,
            scope,
        });
        imported.push(name);
    }

    // 6. One manifest write (only if something changed).
    if !imported.is_empty() {
        fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    }

    // 7. Report.
    for w in &warnings {
        eprintln!("[aenv] warning: {w}");
    }
    for n in &imported {
        println!("  + {n}");
    }
    for n in &skipped {
        println!("  = {n} (already declared)");
    }
    let ref_disp = resolved_ref.as_deref().unwrap_or("local source");
    println!(
        "Imported {} skill{} from {source} @ {ref_disp} into namespace '{namespace}'.",
        imported.len(),
        if imported.len() == 1 { "" } else { "s" }
    );
    if !skipped.is_empty() {
        println!("({} already declared, skipped.)", skipped.len());
    }
    Ok(())
}

/// True when `SKILL.md` bytes begin with a `---` YAML frontmatter block that
/// contains a non-empty `name:` key. Used to skip malformed skill dirs rather
/// than failing the whole bulk import.
fn has_name_frontmatter(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return false;
    }
    let mut lines = trimmed.lines();
    lines.next(); // opening `---`
    for line in lines {
        let l = line.trim();
        if l == "---" {
            break; // end of frontmatter
        }
        if let Some(val) = l.strip_prefix("name:") {
            if !val.trim().is_empty() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::has_name_frontmatter;

    #[test]
    fn detects_valid_name_frontmatter() {
        assert!(has_name_frontmatter(
            b"---\nname: foo\ndescription: x\n---\n# foo\n"
        ));
    }

    #[test]
    fn rejects_missing_frontmatter() {
        assert!(!has_name_frontmatter(b"# foo\nno frontmatter here\n"));
    }

    #[test]
    fn rejects_empty_name() {
        assert!(!has_name_frontmatter(b"---\nname:\ndescription: x\n---\n"));
    }

    #[test]
    fn rejects_name_only_after_frontmatter_close() {
        assert!(!has_name_frontmatter(
            b"---\ndescription: x\n---\nname: late\n"
        ));
    }
}
