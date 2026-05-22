//! Doctor: evaluate every resolved policy against the resolved namespace.
//!
//! Phase 3 produces a `DoctorReport` carrying:
//! * the namespace chain (for the CLI to print "base → leaf")
//! * the resolved policy map (key → ResolvedPolicy)
//! * the flat outcome list (one or more entries per policy key)
//!
//! `aenv doctor` (Task 17) renders this as text. The `enforce_policies_block`
//! function (Task 15) uses `has_enforce_violations()` to short-circuit
//! activation before any file writes.

use crate::adapter::AdapterRegistry;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::policies::builtin::{dispatch, OutcomeStatus, PolicyContext, PolicyOutcome};
use crate::policies::ResolvedPolicy;
use crate::resolve::ResolutionResult;
use std::collections::BTreeMap;

/// The product of evaluating every policy against a resolved namespace.
#[derive(Debug, Clone)]
pub struct DoctorReport {
    /// Root → leaf order.
    pub chain: Vec<NamespaceId>,
    /// Effective policies (qualified by source).
    pub policies: BTreeMap<String, ResolvedPolicy>,
    /// One outcome per (policy, target) pair.
    pub outcomes: Vec<PolicyOutcome>,
}

impl DoctorReport {
    /// Total number of `Fail` outcomes (enforced-policy violations).
    pub fn fail_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, OutcomeStatus::Fail { .. }))
            .count()
    }

    /// Number of `Warn` outcomes (advisory violations).
    pub fn warn_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
            .count()
    }

    /// Whether any enforce-policy violation occurred.
    pub fn has_enforce_violations(&self) -> bool {
        self.fail_count() > 0
    }

    /// One-line summary for human-friendly text output.
    pub fn summary_line(&self) -> String {
        let f = self.fail_count();
        let w = self.warn_count();
        if f == 0 && w == 0 {
            "No issues found.".into()
        } else if f == 0 {
            format!(
                "{w} advisory issue{}; activation unaffected (doctor is advisory).",
                if w == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "{f} enforce-policy violation{}, {w} advisory issue{}.",
                if f == 1 { "" } else { "s" },
                if w == 1 { "" } else { "s" }
            )
        }
    }
}

/// Evaluate every policy in `resolved.policies` against `resolved.candidates`.
pub fn evaluate<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    resolved: &ResolutionResult,
) -> DoctorReport {
    let ctx = PolicyContext {
        fs,
        layout,
        adapters,
        resolved,
    };
    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for (key, policy) in &resolved.policies {
        outcomes.extend(dispatch(key, policy, &ctx));
    }
    DoctorReport {
        chain: resolved.chain.clone(),
        policies: resolved.policies.clone(),
        outcomes,
    }
}
