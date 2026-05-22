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
    ///
    /// Wording matches functional spec §5.12:
    /// - clean: "No issues found."
    /// - advisory only: "N policy violations. Activation is unaffected; doctor is advisory."
    /// - enforced: "N enforce-policy violations, M advisory issues. Activation refused."
    pub fn summary_line(&self) -> String {
        let f = self.fail_count();
        let w = self.warn_count();
        if f == 0 && w == 0 {
            "No issues found.".into()
        } else if f == 0 {
            format!(
                "{w} policy violation{}. Activation is unaffected; doctor is advisory.",
                if w == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "{f} enforce-policy violation{}, {w} advisory issue{}. Activation refused.",
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
    let mut effective_policies = resolved.policies.clone();
    if !effective_policies.contains_key("instructions_max_chars") {
        if let Some(synth) = synthesize_instructions_limit(adapters, resolved) {
            effective_policies.insert("instructions_max_chars".to_string(), synth);
        }
    }

    let ctx = PolicyContext {
        fs,
        layout,
        adapters,
        resolved,
    };
    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for (key, policy) in &effective_policies {
        outcomes.extend(dispatch(key, policy, &ctx));
    }
    DoctorReport {
        chain: resolved.chain.clone(),
        policies: effective_policies,
        outcomes,
    }
}

/// R-24 auto-fire helper: produce an `instructions_max_chars` policy from
/// the strictest adapter `soft_limits.instructions` across the resolved
/// candidates, when no manifest declared one. Returns `None` if no
/// instructions-role candidate has an adapter with a declared soft limit.
///
/// The synthesized policy is always advisory (`enforce = false`) — adapter
/// defaults shouldn't block activation. Attribute the source to the leaf
/// namespace; if the chain is empty (synthetic test contexts) we fall back
/// to a `"(synthesized)"` sentinel.
///
/// **Interaction with R-26 `instructions_budget`:** the per-evaluator
/// narrowing in `instructions_max_chars::evaluate` still applies, so the
/// effective limit is `min(adapter_soft_limit, instructions_budget)`.
fn synthesize_instructions_limit(
    adapters: &AdapterRegistry,
    resolved: &ResolutionResult,
) -> Option<crate::policies::ResolvedPolicy> {
    let mut min_limit: Option<usize> = None;
    for c in &resolved.candidates {
        let Some(adapter) = adapters.get(&c.adapter) else {
            continue;
        };
        let role = adapter
            .roles
            .get(c.path.to_string_lossy().as_ref())
            .map(String::as_str)
            .unwrap_or("");
        if role != "instructions" {
            continue;
        }
        if let Some(&limit) = adapter.soft_limits.get("instructions") {
            min_limit = Some(min_limit.map_or(limit, |m| m.min(limit)));
        }
    }
    let limit = min_limit?;
    let leaf = resolved
        .chain
        .last()
        .cloned()
        .unwrap_or_else(|| crate::identity::NamespaceId::new("(synthesized)").unwrap());
    Some(crate::policies::ResolvedPolicy {
        value: crate::policies::PolicyValue::Integer(limit as i64),
        enforce: false,
        source: leaf,
    })
}
