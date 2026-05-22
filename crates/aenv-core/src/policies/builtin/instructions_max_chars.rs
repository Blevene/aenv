//! `instructions_max_chars`: cap on the UTF-8 character count of any
//! adapter-managed file with `role = "instructions"`.

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::policies::builtin::{PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "instructions_max_chars";

/// Evaluate the policy against every `role = "instructions"` candidate.
pub fn evaluate<F: Filesystem>(
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    let limit = match &policy.value {
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

    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for c in &ctx.resolved.candidates {
        let adapter = match ctx.adapters.get(&c.adapter) {
            Some(a) => a,
            None => continue,
        };
        let role = adapter
            .roles
            .get(c.path.to_string_lossy().as_ref())
            .map(String::as_str)
            .unwrap_or("");
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
        if chars <= limit {
            outcomes.push(PolicyOutcome::pass(KEY, Some(target)));
        } else {
            let msg = format!(
                "{} has {chars} chars (budget {limit}). Refactor procedural content into \
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
    outcomes
}
