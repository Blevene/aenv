//! Schema for `aenv doctor --json`.

use crate::policies::ResolvedPolicy;
use serde::Serialize;
use std::collections::BTreeMap;

/// JSON shape for `aenv doctor --json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DoctorReportJson {
    /// Namespace that was examined.
    pub namespace: String,
    /// Resolution chain from root to leaf.
    pub chain: Vec<String>,
    /// Effective policies after `extends`-chain resolution.
    pub policies: BTreeMap<String, ResolvedPolicy>,
    /// Per-check outcomes.
    pub outcomes: Vec<OutcomeJson>,
    /// Number of checks that passed.
    pub pass_count: usize,
    /// Number of checks that produced warnings.
    pub warn_count: usize,
    /// Number of checks that failed.
    pub fail_count: usize,
    /// Number of checks that were skipped.
    pub skipped_count: usize,
}

/// Result of a single doctor check.
#[derive(Debug, Clone, Default, Serialize)]
pub struct OutcomeJson {
    /// Policy or check key (e.g. `mcp_requires_command_or_url`).
    pub key: String,
    /// `pass`, `warn`, `fail`, or `skipped`.
    pub status: String,
    /// File or resource targeted by this check, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Human-readable message elaborating on the outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}
