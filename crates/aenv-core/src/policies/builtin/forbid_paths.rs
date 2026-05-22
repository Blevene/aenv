//! `forbid_paths`: deny-list of materialized paths.

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::policies::builtin::{PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "forbid_paths";

/// Evaluate every resolved candidate against the patterns. Emits a single
/// `Pass` outcome with no target if nothing matched; emits a Warn/Fail per
/// matching candidate otherwise.
pub fn evaluate<F: Filesystem>(
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    let patterns: &Vec<String> = match &policy.value {
        PolicyValue::ListString(xs) => xs,
        _ => {
            return vec![PolicyOutcome::warn_skip(
                KEY,
                format!(
                    "policy '{KEY}' must be a list-of-string; got {} (source: {})",
                    policy.value.type_tag(),
                    policy.source
                ),
            )];
        }
    };

    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for c in &ctx.resolved.candidates {
        let rel = c.path.to_string_lossy().to_string();
        let hit = patterns.iter().any(|p| forbid_match(p, &rel));
        if !hit {
            continue;
        }
        let target = QualifiedName::new(
            c.namespace.clone(),
            ShortName::new(rel.clone())
                .unwrap_or_else(|_| ShortName::new("?".to_string()).expect("trivial short name is valid")),
        );
        let msg = format!(
            "{} matches forbid_paths pattern; namespace must not declare this path.",
            rel
        );
        outcomes.push(if policy.enforce {
            PolicyOutcome::fail(KEY, Some(target), msg)
        } else {
            PolicyOutcome::warn(KEY, Some(target), msg)
        });
    }
    if outcomes.is_empty() {
        outcomes.push(PolicyOutcome::pass(KEY, None));
    }
    outcomes
}

fn forbid_match(pattern: &str, candidate: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        candidate.starts_with(prefix) && candidate[prefix.len()..].starts_with('/')
    } else if let Some(prefix) = pattern.strip_suffix("/**/*") {
        candidate.starts_with(prefix) && candidate[prefix.len()..].starts_with('/')
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        candidate.starts_with(prefix)
    } else {
        pattern == candidate
    }
}
