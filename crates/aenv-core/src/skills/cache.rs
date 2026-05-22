//! Cache directory layout for fetched skills.
//!
//! Fetched (imported) skills are cached under
//! `AENV_HOME/cache/skills/<source-hash>/<ref>/<files...>`.
//! `<source-hash>` is the first `CACHE_KEY_BYTES * 2` hex chars of
//! SHA-256(source-string) — collision-resistant enough that two different
//! sources will never share a directory in practice. `<ref>` is the resolved
//! git ref or the literal `"head"` for unpinned sources at first resolution.

use crate::home::RegistryLayout;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Number of leading SHA-256 bytes (rendered as 2 hex chars each) used as
/// the cache directory key. 8 bytes → 16 hex chars → 2^64 collision space,
/// well beyond the O(100) realistic source count.
const CACHE_KEY_BYTES: usize = 8;

/// Stable hex-encoded SHA-256 of `bytes`. Used for `resolved_hash` provenance
/// on skill source resolutions (full 64-char hex).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Stable short hash of a source string (the first `CACHE_KEY_BYTES * 2` hex
/// chars of SHA-256). Used as the cache directory name; intentionally
/// truncated for readable paths.
pub fn source_hash(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher
        .finalize()
        .iter()
        .take(CACHE_KEY_BYTES)
        .map(|b| format!("{b:02x}"))
        .collect()
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
