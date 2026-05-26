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
        let label = match c.scope {
            crate::scope::Scope::Project => rel.clone(),
            crate::scope::Scope::User => format!("~/{rel}"),
        };
        let target = QualifiedName::new(
            c.namespace.clone(),
            ShortName::new(label.clone()).unwrap_or_else(|_| {
                ShortName::new("?".to_string()).expect("trivial short name is valid")
            }),
        );
        let msg =
            format!("{label} matches forbid_paths pattern; namespace must not declare this path.");
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

/// Match `candidate` against a single `forbid_paths` pattern.
///
/// Supported forms (no nesting, no `?` or character classes):
///   * `secrets/**` — matches any path under `secrets/` (one or more components).
///   * `.env*` — matches any path whose first component begins with `.env`.
///   * literal — exact string match.
fn forbid_match(pattern: &str, candidate: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        candidate.starts_with(prefix) && candidate[prefix.len()..].starts_with('/')
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        candidate.starts_with(prefix)
    } else {
        pattern == candidate
    }
}
