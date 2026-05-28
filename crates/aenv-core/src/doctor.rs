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
use crate::identity::{NamespaceId, QualifiedName, ShortName};
use crate::policies::builtin::{dispatch, OutcomeStatus, PolicyContext, PolicyOutcome};
use crate::policies::ResolvedPolicy;
use crate::resolve::ResolutionResult;
use std::collections::BTreeMap;
use std::path::Path;

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
///
/// `target_root` is the activation root that the synthetic `hook_paths_resolvable`
/// pre-flight check uses to resolve `$HOME` / `$AENV_TARGET_ROOT` references in
/// settings.json command strings. Project-side doctor passes the project root;
/// global doctor passes `$HOME`; the activator passes whatever scope it's
/// activating into. Callers who don't care about the pre-flight check (mostly
/// older test fixtures) may pass any path — settings.json without
/// command-shaped paths simply produces no `hook_paths_resolvable` outcomes.
pub fn evaluate<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    resolved: &ResolutionResult,
    target_root: &Path,
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

    // Synthetic policy: `hook_paths_resolvable`. Auto-fires for every
    // settings.json candidate; never declared by manifests; advisory only
    // (no Fail status, never blocks activation). Surfaces hook / MCP /
    // statusLine command paths that wouldn't resolve on disk after
    // activation — the F5 lockout class.
    outcomes.extend(synthesize_preflight_outcomes(fs, target_root, resolved));

    // Synthetic policy: `copy_mode_local_edits`. Compares each Copy-strategy
    // managed file's current on-disk bytes against the resolved expected
    // bytes; warns when the user has edited the target since the last
    // activation. Advisory only (no Fail status); requires that the active
    // global state describes the namespace being doctored.
    outcomes.extend(synthesize_copy_drift_outcomes(
        fs,
        layout,
        adapters,
        target_root,
        resolved,
    ));

    DoctorReport {
        chain: resolved.chain.clone(),
        policies: effective_policies,
        outcomes,
    }
}

/// Build `hook_paths_resolvable` outcomes from a pre-flight scan.
///
/// One `Warn` per missing path. `target` carries a synthesized qualified name
/// of the form `<leaf-ns>::<rendered-settings-path>` so it renders in the
/// existing doctor outputs without special cases. Returns an empty vec if the
/// scanner finds nothing (or hits a filesystem error — pre-flight is best-effort).
fn synthesize_preflight_outcomes<F: Filesystem>(
    fs: &F,
    target_root: &Path,
    resolved: &ResolutionResult,
) -> Vec<PolicyOutcome> {
    let findings = match crate::preflight::preflight_settings_commands(
        fs,
        target_root,
        &resolved.candidates,
    ) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    if findings.is_empty() {
        return Vec::new();
    }
    let leaf = resolved
        .chain
        .last()
        .cloned()
        .unwrap_or_else(|| NamespaceId::new("(synthesized)").unwrap());
    let mut out = Vec::with_capacity(findings.len());
    for f in findings {
        // Render the settings path as `~/path` for user-scope target roots
        // (matches the existing `target_label` convention). Anything else
        // renders as the absolute display.
        let target_label = render_target_under_root(&f.settings_path, target_root);
        // Construct a `QualifiedName`. If `target_label` is rejected by the
        // ShortName validator (contains "::") fall back to an opaque label;
        // shouldn't happen for filesystem paths in practice.
        let short = ShortName::new(target_label.clone())
            .unwrap_or_else(|_| ShortName::new("settings.json").unwrap());
        let qn = QualifiedName::new(leaf.clone(), short);
        out.push(PolicyOutcome::warn(
            "hook_paths_resolvable",
            Some(qn),
            format!(
                "{} hook in {} references {} (missing). Hint: run the namespace's install \
                 procedure (e.g. on_activate) or declare the runtime path in user_files.",
                f.kind.as_label(),
                target_label,
                f.missing_path.display(),
            ),
        ));
    }
    out
}

/// Build `copy_mode_local_edits` outcomes by comparing each Copy-strategy
/// managed file's current on-disk bytes to the resolved expected bytes.
///
/// Returns no outcomes if there is no active global state file, if the
/// active state describes a different namespace than the one being doctored,
/// or if the resolved namespace has no Copy-strategy managed files. Each
/// divergent file produces one `Warn` outcome.
///
/// The synthesized target is `<leaf-namespace>::~/<path>` so the global
/// doctor's existing `::~/` user-scope filter (Task 17) admits these
/// outcomes without special-casing.
fn synthesize_copy_drift_outcomes<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    target_root: &Path,
    resolved: &ResolutionResult,
) -> Vec<PolicyOutcome> {
    use crate::resolve::MaterializeStrategy;

    // 1. Load active state, if any. The drift class only exists for an
    //    already-activated namespace; with no state file there's nothing
    //    to compare against.
    let state_path = layout.global_state_path();
    if !fs.exists(&state_path).unwrap_or(false) {
        return Vec::new();
    }
    let body = match fs.read(&state_path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let text = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let state = match crate::state::ActivationState::from_json(text) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    // 2. Only fire when the active namespace matches the namespace being
    //    doctored. Doctor on a non-active namespace is a different question.
    let leaf = match resolved.chain.last() {
        Some(l) => l.clone(),
        None => return Vec::new(),
    };
    if state.active_namespace != leaf.as_str() {
        return Vec::new();
    }

    // 3. Collect Copy-strategy managed files. Symlink targets don't have
    //    this drift class (edits flow back to the namespace source).
    let copy_files: Vec<&crate::state::ManagedFile> = state
        .managed_files
        .iter()
        .filter(|m| matches!(m.strategy, MaterializeStrategy::Copy))
        .collect();
    if copy_files.is_empty() {
        return Vec::new();
    }

    // 4. Compute expected bytes via the resolved user-scope material set.
    //    Best-effort: any failure here means we can't tell whether the
    //    on-disk bytes drifted, so we stay silent rather than guess.
    let mat = match crate::materialize::compute_material_set_user(fs, layout, adapters, &leaf) {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };

    // 5. Compare each Copy managed file's on-disk bytes to expected. A
    //    missing file is a different problem (handled elsewhere); skip it.
    let mut outcomes = Vec::new();
    for m in copy_files {
        let on_disk = target_root.join(&m.path);
        let actual = match fs.read(&on_disk) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let expected = match mat.entries().iter().find(|(p, _)| p == &m.path) {
            Some((_, bytes)) => bytes,
            None => continue,
        };
        if &actual != expected {
            let label = format!("~/{}", m.path.display());
            let short = ShortName::new(label.clone())
                .unwrap_or_else(|_| ShortName::new("copy-target").unwrap());
            let qn = QualifiedName::new(leaf.clone(), short);
            outcomes.push(PolicyOutcome::warn(
                "copy_mode_local_edits",
                Some(qn),
                format!(
                    "{label} has been edited locally since activation; next activation \
                     will overwrite your edits. Run `aenv global snapshot <name>` first to capture."
                ),
            ));
        }
    }
    outcomes
}

/// Render an absolute settings.json path relative to `target_root` as
/// `~/<rest>` when target_root is a prefix; otherwise return its display.
fn render_target_under_root(settings_path: &Path, target_root: &Path) -> String {
    if let Ok(rel) = settings_path.strip_prefix(target_root) {
        format!("~/{}", rel.display())
    } else {
        settings_path.display().to_string()
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
        let (roles_map, limits_map, lookup_key) = match c.scope {
            crate::scope::Scope::Project => (
                &adapter.roles,
                &adapter.soft_limits,
                c.path.to_string_lossy().into_owned(),
            ),
            crate::scope::Scope::User => (
                &adapter.user_roles,
                &adapter.user_soft_limits,
                format!("~/{}", c.path.display()),
            ),
        };
        let role = roles_map
            .get(lookup_key.as_str())
            .map_or("", String::as_str);
        if role != "instructions" {
            continue;
        }
        if let Some(&limit) = limits_map.get("instructions") {
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
