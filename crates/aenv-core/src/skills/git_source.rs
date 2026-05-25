//! Resolve a git-URL skill source into a cached directory.
//!
//! Pinned sources cache under `<source-hash>/<ref>/`; unpinned sources
//! cache under `<source-hash>/head/`. A pre-existing cache directory is
//! reused (the `aenv skill refresh` command, deferred from Phase 4, will
//! invalidate it).

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::skills::cache::sha256_hex;
use crate::skills::cache::skill_cache_path;
use crate::skills::git::git_clone;
use crate::skills::local::ResolvedSkill;
use std::path::Path;

/// Result is `ResolvedSkill` because, once cloned, a git source behaves
/// like a local-path source for materialization purposes.
///
/// When `sub_path` is `Some(rel)`, the returned `source_path` is
/// `<cache_dir>/<rel>` (used to import a single skill from a monorepo like
/// `scientific-skills/<name>/SKILL.md`). Otherwise `source_path` is the
/// clone root and the SKILL.md lookup falls back to
/// `<cache_dir>/<skill_name>/SKILL.md` or `<cache_dir>/SKILL.md`.
pub fn resolve_git<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    url: &str,
    ref_spec: Option<&str>,
    skill_name: &str,
    sub_path: Option<&str>,
) -> Result<ResolvedSkill> {
    let ref_label = ref_spec.unwrap_or("head").to_string();
    let cache_dir = skill_cache_path(layout, url, &ref_label);

    if !fs.exists(&cache_dir)? {
        // Not cached. Create parent, clone, then read.
        if let Some(parent) = cache_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AenvError::Io(std::io::Error::new(
                    e.kind(),
                    format!("create cache parent {}: {e}", parent.display()),
                ))
            })?;
        }
        git_clone(url, ref_spec, &cache_dir)?;
    }

    let resolved_sha = git_head_sha(&cache_dir)?;
    let source_path = match sub_path {
        Some(p) => cache_dir.join(p),
        None => cache_dir.clone(),
    };
    let resolved_hash = compute_skill_hash(fs, &cache_dir, skill_name, sub_path)?;

    Ok(ResolvedSkill {
        source_path,
        resolved_ref: Some(resolved_sha),
        resolved_hash,
    })
}

/// Compute `"sha256:<hex>"` of the skill's `SKILL.md` bytes.
///
/// When `sub_path` is set, looks for `<cache_dir>/<sub_path>/SKILL.md` and
/// errors if absent — explicit path means "the SKILL.md is exactly there."
/// When unset, falls back to `<cache_dir>/<skill_name>/SKILL.md` then
/// `<cache_dir>/SKILL.md` (legacy single-skill repos).
fn compute_skill_hash<F: Filesystem>(
    fs: &F,
    cache_dir: &Path,
    skill_name: &str,
    sub_path: Option<&str>,
) -> Result<String> {
    let skill_md = if let Some(rel) = sub_path {
        let candidate = cache_dir.join(rel).join("SKILL.md");
        if !fs.exists(&candidate)? {
            return Err(AenvError::ManifestInvalid(format!(
                "git source at {} has no SKILL.md under '{rel}/'",
                cache_dir.display()
            )));
        }
        candidate
    } else {
        let subdir_skill_md = cache_dir.join(skill_name).join("SKILL.md");
        let root_skill_md = cache_dir.join("SKILL.md");
        if fs.exists(&subdir_skill_md)? {
            subdir_skill_md
        } else if fs.exists(&root_skill_md)? {
            root_skill_md
        } else {
            return Err(AenvError::ManifestInvalid(format!(
                "git source at {} has no SKILL.md under '{skill_name}/' or at root",
                cache_dir.display()
            )));
        }
    };

    let bytes = fs.read(&skill_md)?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}

fn git_head_sha(dir: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| AenvError::RemoteUnreachable(format!("git rev-parse: {e}")))?;
    if !output.status.success() {
        return Err(AenvError::RemoteUnreachable(format!(
            "git rev-parse HEAD in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
