//! Resolved-namespace content hash (PRD §5.17, R-84–R-87).
//!
//! The hash is computed from a *material set* (the post-merge byte
//! contents that would be written to disk on activation) plus a synthetic
//! `.aenv/parameters.json` entry carrying the resolved parameter map.
//!
//! Implementation lands in Task 4. This file exists so dependent modules
//! can `use crate::hash;` without a chain of dependent diffs.

#![allow(dead_code)] // Filled in by Task 4.

/// Algorithm-version byte prepended to the hash input. Bumping this is a
/// breaking change per R-87 and requires a dual-emit deprecation window.
pub(crate) const ALGORITHM_VERSION_V1: u8 = 0x01;

/// User-facing prefix advertised on every emitted hash string.
pub const HASH_PREFIX_V1: &str = "sha256-v1:";
