//! `instructions_max_chars`: cap on the UTF-8 character count of any
//! adapter-managed file with `role = "instructions"`.
//!
//! Effective limit: `min(policy_limit, instructions_budget_parameter)`.
//! If `instructions_budget` is declared in a manifest but no
//! `instructions_max_chars` policy is declared AND no adapter has a
//! `soft_limits.instructions` entry, the budget parameter has no effect —
//! the policy never fires. This is intentional: the parameter narrows
//! limits, it does not create them. Users who want a limit from the budget
//! alone should also declare the policy (or rely on a built-in adapter's
//! `soft_limits.instructions` triggering R-24 auto-fire in `doctor.rs`).

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::parameters::ParameterValue;
use crate::policies::builtin::{PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "instructions_max_chars";

/// Evaluate the policy against every `role = "instructions"` candidate.
pub fn evaluate<F: Filesystem>(
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    let policy_limit = match &policy.value {
        PolicyValue::Integer(n) if *n >= 0 => *n as usize,
        _ => {
            return vec![PolicyOutcome::warn_skip(
                KEY,
                format!(
                    "policy '{KEY}' must be a non-negative integer; got {} (source: {})",
                    policy.value.type_tag(),
                    policy.source
                ),
            )];
        }
    };
    let budget_limit = match ctx.resolved.parameters.get("instructions_budget") {
        Some(rp) => match &rp.value {
            ParameterValue::Integer(n) if *n >= 0 => Some(*n as usize),
            _ => None,
        },
        None => None,
    };
    let effective = match budget_limit {
        Some(b) => policy_limit.min(b),
        None => policy_limit,
    };

    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for c in &ctx.resolved.candidates {
        let adapter = match ctx.adapters.get(&c.adapter) {
            Some(a) => a,
            None => continue,
        };
        let role = adapter
            .roles
            .get(c.path.to_string_lossy().as_ref())
            .map_or("", String::as_str);
        if role != "instructions" {
            continue;
        }
        let target = QualifiedName::new(
            c.namespace.clone(),
            ShortName::new(c.path.to_string_lossy().to_string()).unwrap_or_else(|_| {
                ShortName::new("?".to_string()).expect("trivial short name is valid")
            }),
        );

        let bytes = match ctx.fs.read(&c.source_path) {
            Ok(b) => b,
            Err(e) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!(
                        "cannot read instructions file {}: {e}",
                        c.source_path.display()
                    ),
                ));
                continue;
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!(
                        "instructions file {} is not valid UTF-8; cannot count chars",
                        c.source_path.display()
                    ),
                ));
                continue;
            }
        };
        let chars = text.chars().count();
        if chars <= effective {
            outcomes.push(PolicyOutcome::pass(KEY, Some(target)));
        } else {
            let msg = format!(
                "{} has {chars} chars (budget {effective}). Refactor procedural content into \
                 skills, dispositional content into subagents, or use @-imports.",
                c.path.display()
            );
            outcomes.push(if policy.enforce {
                PolicyOutcome::fail(KEY, Some(target), msg)
            } else {
                PolicyOutcome::warn(KEY, Some(target), msg)
            });
        }
    }
    // Match `forbid_paths`'s convention: emit a single targetless Pass when no
    // candidate applies, so `aenv doctor` always shows a signal per evaluated
    // policy rather than silently emitting nothing.
    if outcomes.is_empty() {
        outcomes.push(PolicyOutcome::pass(KEY, None));
    }
    outcomes
}
