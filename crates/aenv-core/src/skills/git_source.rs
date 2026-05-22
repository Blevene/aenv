//! Resolve a git-URL skill source into a cached directory.
//!
//! Pinned sources cache under `<source-hash>/<ref>/`; unpinned sources
//! cache under `<source-hash>/head/`. A pre-existing cache directory is
//! reused (the `aenv skill refresh` command, deferred from Phase 4, will
//! invalidate it).

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::skills::cache::skill_cache_path;
use crate::skills::git::{git_clone, git_resolve_ref};
use crate::skills::local::LocalResolution;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Result is `LocalResolution` because, once cloned, a git source behaves
/// like a local-path source for materialization purposes.
///
/// `source_path` is always the clone root (`cache_dir`). The content hash is
/// derived from whichever `SKILL.md` is found: `<cache_dir>/<skill_name>/SKILL.md`
/// first, then `<cache_dir>/SKILL.md` as a fallback.
pub fn resolve_git<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    url: &str,
    ref_spec: Option<&str>,
    skill_name: &str,
) -> Result<LocalResolution> {
    let ref_label = ref_spec.unwrap_or("head").to_string();
    let cache_dir = skill_cache_path(layout, url, &ref_label);

    if fs.exists(&cache_dir)? {
        // Cached. Read the resolved SHA from the existing clone via shell-out.
        let resolved_sha = git_head_sha(&cache_dir)?;
        let resolved_hash = compute_skill_hash(fs, &cache_dir, skill_name)?;
        return Ok(LocalResolution {
            source_path: cache_dir,
            resolved_ref: Some(resolved_sha),
            resolved_hash,
        });
    }

    // Not cached. Create parent, clone, then read.
    if let Some(parent) = cache_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AenvError::Io(std::io::Error::new(
                e.kind(),
                format!("create cache parent {}: {e}", parent.display()),
            ))
        })?;
    }
    let resolved_sha = git_clone(url, ref_spec, &cache_dir)?;
    let _ = git_resolve_ref; // intentionally unused: clone returns the SHA we need

    let resolved_hash = compute_skill_hash(fs, &cache_dir, skill_name)?;

    Ok(LocalResolution {
        source_path: cache_dir,
        resolved_ref: Some(resolved_sha),
        resolved_hash,
    })
}

/// Compute `"sha256:<hex>"` of the skill's `SKILL.md` bytes.
///
/// Looks for `<cache_dir>/<skill_name>/SKILL.md` first, then
/// `<cache_dir>/SKILL.md` as a fallback (repos that put the skill at root).
fn compute_skill_hash<F: Filesystem>(
    fs: &F,
    cache_dir: &Path,
    skill_name: &str,
) -> Result<String> {
    let subdir_skill_md = cache_dir.join(skill_name).join("SKILL.md");
    let root_skill_md = cache_dir.join("SKILL.md");

    let skill_md = if fs.exists(&subdir_skill_md)? {
        subdir_skill_md
    } else if fs.exists(&root_skill_md)? {
        root_skill_md
    } else {
        return Err(AenvError::ManifestInvalid(format!(
            "git source at {} has no SKILL.md under '{skill_name}/' or at root",
            cache_dir.display()
        )));
    };

    let bytes = fs.read(&skill_md)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("sha256:{hex}"))
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
