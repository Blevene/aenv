//! Schema for `aenv doctor --json`.

use crate::doctor::DoctorReport;
use crate::policies::builtin::OutcomeStatus;
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

impl DoctorReportJson {
    /// Build a `DoctorReportJson` from an in-memory `DoctorReport`.
    pub fn from_report(namespace: &str, report: &DoctorReport) -> Self {
        let mut pass_count = 0;
        let mut warn_count = 0;
        let mut fail_count = 0;
        let mut skipped_count = 0;
        let outcomes: Vec<OutcomeJson> = report
            .outcomes
            .iter()
            .map(|o| {
                let (status, msg) = match &o.status {
                    OutcomeStatus::Pass => {
                        pass_count += 1;
                        ("pass", None)
                    }
                    OutcomeStatus::Warn { msg } => {
                        warn_count += 1;
                        ("warn", Some(msg.clone()))
                    }
                    OutcomeStatus::Fail { msg } => {
                        fail_count += 1;
                        ("fail", Some(msg.clone()))
                    }
                    OutcomeStatus::WarnSkip { msg } => {
                        skipped_count += 1;
                        ("skipped", Some(msg.clone()))
                    }
                };
                OutcomeJson {
                    key: o.key.clone(),
                    status: status.to_string(),
                    target: o.target.as_ref().map(ToString::to_string),
                    msg,
                }
            })
            .collect();
        DoctorReportJson {
            namespace: namespace.to_string(),
            chain: report
                .chain
                .iter()
                .map(|n| n.as_str().to_string())
                .collect(),
            policies: report.policies.clone(),
            outcomes,
            pass_count,
            warn_count,
            fail_count,
            skipped_count,
        }
    }
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
