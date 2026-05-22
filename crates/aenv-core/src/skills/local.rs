//! Resolve a local-path skill source.
//!
//! Local sources don't need a cache — the path on disk IS the source. We
//! only verify that `SKILL.md` exists under the given directory and compute
//! a content hash for state-file provenance.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The output shape every skill source resolver produces. Phase 4 git and
/// registry resolvers reuse this struct so callers can dispatch on
/// `SourceKind` without dealing with three different result types.
#[derive(Debug)]
pub struct LocalResolution {
    /// Absolute source directory.
    pub source_path: PathBuf,
    /// For git sources: the resolved commit SHA. `None` for local sources.
    pub resolved_ref: Option<String>,
    /// `"sha256:<hex>"` of the SKILL.md body.
    pub resolved_hash: String,
}

/// Validate that `<source_dir>/SKILL.md` exists and hash its bytes.
pub fn resolve_local<F: Filesystem>(
    fs: &F,
    source_dir: &Path,
    _skill_name: &str,
) -> Result<LocalResolution> {
    if !fs.exists(source_dir)? {
        return Err(AenvError::ManifestInvalid(format!(
            "local skill source directory does not exist: {}",
            source_dir.display()
        )));
    }
    let skill_md = source_dir.join("SKILL.md");
    if !fs.exists(&skill_md)? {
        return Err(AenvError::ManifestInvalid(format!(
            "local skill source {} has no SKILL.md",
            source_dir.display()
        )));
    }
    let bytes = fs.read(&skill_md)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(LocalResolution {
        source_path: source_dir.to_path_buf(),
        resolved_ref: None,
        resolved_hash: format!("sha256:{hex}"),
    })
}
