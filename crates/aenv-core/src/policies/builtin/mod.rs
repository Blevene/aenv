//! Built-in policy evaluators.
//!
//! Each built-in policy key (`instructions_max_chars`,
//! `skill_requires_description`, `mcp_requires_command_or_url`,
//! `forbid_paths`) ships as a dedicated evaluator. The `dispatch` function
//! routes a resolved policy to its evaluator; unknown keys produce a single
//! `WarnSkip` outcome so `aenv doctor` can report them without failing.
//!
//! `PolicyContext` carries the references an evaluator needs without forcing
//! every evaluator to take a long argument list.

pub mod forbid_paths;
pub mod instructions_max_chars;
pub mod mcp_requires_command_or_url;
pub mod skill_requires_description;

use crate::adapter::AdapterRegistry;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::identity::QualifiedName;
use crate::policies::ResolvedPolicy;
use crate::resolve::ResolutionResult;

/// References an evaluator needs to walk the namespace + its artifacts.
pub struct PolicyContext<'a, F: Filesystem> {
    /// Filesystem the evaluator should read through.
    pub fs: &'a F,
    /// Registry layout (paths to namespace dirs, manifest paths).
    pub layout: &'a RegistryLayout,
    /// Adapter registry (for `role = "instructions"` lookup).
    pub adapters: &'a AdapterRegistry,
    /// The resolved chain whose artifacts we evaluate.
    pub resolved: &'a ResolutionResult,
}

/// One result emitted by an evaluator.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PolicyOutcome {
    /// The policy key that produced this outcome.
    pub key: String,
    /// The artifact this outcome talks about, when applicable.
    pub target: Option<QualifiedName>,
    /// Pass / Warn / Fail / WarnSkip.
    pub status: OutcomeStatus,
}

/// Per-outcome status.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OutcomeStatus {
    /// The policy is satisfied.
    Pass,
    /// Soft (advisory) violation.
    Warn {
        /// Human-readable explanation / hint.
        msg: String,
    },
    /// Hard (enforced) violation.
    Fail {
        /// Human-readable explanation / hint.
        msg: String,
    },
    /// The evaluator could not run; `aenv doctor` prints a warning and moves on.
    WarnSkip {
        /// Why the evaluator skipped.
        msg: String,
    },
}

impl PolicyOutcome {
    /// Construct a passing outcome.
    pub fn pass(key: impl Into<String>, target: Option<QualifiedName>) -> Self {
        Self {
            key: key.into(),
            target,
            status: OutcomeStatus::Pass,
        }
    }
    /// Construct a warning (advisory violation).
    pub fn warn(
        key: impl Into<String>,
        target: Option<QualifiedName>,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            target,
            status: OutcomeStatus::Warn { msg: msg.into() },
        }
    }
    /// Construct a hard failure (enforce-policy violation).
    pub fn fail(
        key: impl Into<String>,
        target: Option<QualifiedName>,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            target,
            status: OutcomeStatus::Fail { msg: msg.into() },
        }
    }
    /// Construct a "skipped" outcome (unknown key, evaluator unavailable).
    pub fn warn_skip(key: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            target: None,
            status: OutcomeStatus::WarnSkip { msg: msg.into() },
        }
    }
}

/// The interface implemented by every built-in evaluator.
///
/// `evaluate` returns the outcomes for *this* policy against the namespace
/// in context. Returning an empty Vec is legal (e.g. a policy with no
/// applicable artifacts).
pub trait PolicyEvaluator<F: Filesystem> {
    /// Evaluate the policy and produce a flat list of outcomes.
    fn evaluate(&self, policy: &ResolvedPolicy, ctx: &PolicyContext<F>) -> Vec<PolicyOutcome>;
}

/// Route a resolved policy to its evaluator.
pub fn dispatch<F: Filesystem>(
    key: &str,
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    match key {
        "forbid_paths" => forbid_paths::evaluate(policy, ctx),
        "instructions_max_chars" => instructions_max_chars::evaluate(policy, ctx),
        "mcp_requires_command_or_url" => mcp_requires_command_or_url::evaluate(policy, ctx),
        "skill_requires_description" => skill_requires_description::evaluate(policy, ctx),
        other => vec![PolicyOutcome::warn_skip(
            other.to_owned(),
            format!("no built-in evaluator for policy key '{other}'"),
        )],
    }
}

impl<'a> PolicyContext<'a, crate::fs::MockFilesystem> {
    /// Dummy context for tests that don't actually exercise the evaluator —
    /// used by the scaffold test and any future fast-path tests.
    ///
    /// Leaks small values to satisfy lifetime bounds. Only call from tests.
    #[doc(hidden)]
    pub fn dummy() -> Self {
        // Note: leaks owned values to give a `'static` borrow. Only call from
        // tests. The leaks are negligible (one Filesystem, one RegistryLayout,
        // one AdapterRegistry, one ResolutionResult per call).
        let fs: &'static crate::fs::MockFilesystem =
            Box::leak(Box::new(crate::fs::MockFilesystem::new()));
        let layout: &'static crate::home::RegistryLayout =
            Box::leak(Box::new(crate::home::RegistryLayout::new(
                std::path::PathBuf::from("/dummy"),
            )));
        let adapters: &'static crate::adapter::AdapterRegistry =
            Box::leak(Box::new(crate::adapter::AdapterRegistry::new()));
        let resolved: &'static ResolutionResult = Box::leak(Box::new(ResolutionResult {
            chain: vec![],
            candidates: vec![],
            parameters: std::collections::BTreeMap::new(),
            policies: std::collections::BTreeMap::new(),
        }));
        PolicyContext {
            fs,
            layout,
            adapters,
            resolved,
        }
    }
}
