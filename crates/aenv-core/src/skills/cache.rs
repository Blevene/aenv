//! Cache directory layout for fetched skills.
//!
//! Fetched (imported) skills are cached under
//! `AENV_HOME/cache/skills/<source-hash>/<ref>/<files...>`.
//! `<source-hash>` is the first 16 hex chars of SHA-256(source-string) —
//! collision-resistant enough that two different sources will never share
//! a directory in practice. `<ref>` is the resolved git ref or the literal
//! `"head"` for unpinned sources at first resolution.

use crate::home::RegistryLayout;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Stable 16-hex-char hash of a source string.
pub fn source_hash(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let bytes = hasher.finalize();
    let hex: String = bytes.iter().take(8).map(|b| format!("{b:02x}")).collect();
    hex
}

/// Absolute path to the cached content for a (source, ref) pair.
///
/// Does NOT create the directory; callers materialize as needed.
pub fn skill_cache_path(layout: &RegistryLayout, source: &str, ref_label: &str) -> PathBuf {
    layout
        .skills_cache_dir()
        .join(source_hash(source))
        .join(ref_label)
}
