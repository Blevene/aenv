//! Resolve a local-path skill source.
//!
//! Local sources don't need a cache — the path on disk IS the source. We
//! only verify that `SKILL.md` exists under the given directory and compute
//! a content hash for state-file provenance.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::skills::cache::sha256_hex;
use std::path::{Path, PathBuf};

/// The output shape every skill source resolver produces. Phase 4 git and
/// registry resolvers reuse this struct so callers can dispatch on
/// `SourceKind` without dealing with three different result types.
#[derive(Debug)]
pub struct ResolvedSkill {
    /// Absolute source directory.
    pub source_path: PathBuf,
    /// For git sources: the resolved commit SHA. `None` for local sources.
    pub resolved_ref: Option<String>,
    /// `"sha256:<hex>"` of the SKILL.md body.
    pub resolved_hash: String,
}

/// Validate that `<source_dir>/<sub_path?>/SKILL.md` exists and hash its
/// bytes. When `sub_path` is set, the returned `source_path` points at the
/// sub-directory so the materialization walk doesn't pull in unrelated
/// siblings from a monorepo layout.
pub fn resolve_local<F: Filesystem>(
    fs: &F,
    source_dir: &Path,
    _skill_name: &str,
    sub_path: Option<&str>,
) -> Result<ResolvedSkill> {
    if !fs.exists(source_dir)? {
        return Err(AenvError::ManifestInvalid(format!(
            "local skill source directory does not exist: {}",
            source_dir.display()
        )));
    }
    let effective_dir = match sub_path {
        Some(rel) => source_dir.join(rel),
        None => source_dir.to_path_buf(),
    };
    if let Some(rel) = sub_path {
        if !fs.exists(&effective_dir)? {
            return Err(AenvError::ManifestInvalid(format!(
                "local skill source {} has no '{rel}' sub-directory",
                source_dir.display()
            )));
        }
    }
    let skill_md = effective_dir.join("SKILL.md");
    if !fs.exists(&skill_md)? {
        return Err(AenvError::ManifestInvalid(format!(
            "local skill source {} has no SKILL.md",
            effective_dir.display()
        )));
    }
    let bytes = fs.read(&skill_md)?;
    Ok(ResolvedSkill {
        source_path: effective_dir,
        resolved_ref: None,
        resolved_hash: format!("sha256:{}", sha256_hex(&bytes)),
    })
}
