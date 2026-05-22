//! `skill_requires_description`: every authored skill file's YAML frontmatter
//! must declare a non-empty `description:` field.

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::policies::builtin::{PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "skill_requires_description";

/// Evaluate the policy against every resolved candidate that looks like a
/// skill file: relative path matches `.claude/skills/<dir>/SKILL.md`.
pub fn evaluate<F: Filesystem>(
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    let active = match &policy.value {
        PolicyValue::Boolean(b) => *b,
        _ => {
            return vec![PolicyOutcome::warn_skip(
                KEY,
                format!(
                    "policy '{KEY}' must be a boolean; got {} (source: {})",
                    policy.value.type_tag(),
                    policy.source
                ),
            )];
        }
    };
    if !active {
        return Vec::new();
    }

    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for c in &ctx.resolved.candidates {
        if !looks_like_skill_file(&c.path) {
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
                    format!("cannot read {}: {e}", c.source_path.display()),
                ));
                continue;
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!("{} is not valid UTF-8", c.source_path.display()),
                ));
                continue;
            }
        };
        match extract_description(text) {
            DescriptionResult::Present => outcomes.push(PolicyOutcome::pass(KEY, Some(target))),
            other => {
                let msg = match other {
                    DescriptionResult::NoFrontmatter => format!(
                        "{}: no YAML frontmatter found. Add a '---' block at the top with 'name:' and 'description:'.",
                        c.path.display()
                    ),
                    DescriptionResult::Missing => format!(
                        "{}: frontmatter is missing 'description:'. The description tells the model when to invoke the skill.",
                        c.path.display()
                    ),
                    DescriptionResult::Empty => format!(
                        "{}: 'description:' field is empty. Add a one-sentence description so the model knows when to invoke this skill.",
                        c.path.display()
                    ),
                    DescriptionResult::Present => unreachable!(),
                };
                outcomes.push(if policy.enforce {
                    PolicyOutcome::fail(KEY, Some(target), msg)
                } else {
                    PolicyOutcome::warn(KEY, Some(target), msg)
                });
            }
        }
    }
    // Emit a targetless Pass when no skill files matched, so `aenv doctor`
    // always shows a signal per evaluated policy.
    if outcomes.is_empty() {
        outcomes.push(PolicyOutcome::pass(KEY, None));
    }
    outcomes
}

fn looks_like_skill_file(rel: &std::path::Path) -> bool {
    let s = rel.to_string_lossy();
    // Match `.claude/skills/<one component>/SKILL.md` exactly.
    let parts: Vec<&str> = s.split('/').collect();
    parts.len() == 4
        && parts[0] == ".claude"
        && parts[1] == "skills"
        && !parts[2].is_empty()
        && parts[3] == "SKILL.md"
}

#[derive(Debug)]
enum DescriptionResult {
    NoFrontmatter,
    Missing,
    Empty,
    Present,
}

fn extract_description(text: &str) -> DescriptionResult {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return DescriptionResult::NoFrontmatter;
    }
    let mut found: Option<String> = None;
    for line in lines.by_ref() {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("description:") {
            found = Some(rest.trim().to_string());
        }
    }
    match found {
        Some(v) if v.is_empty() => DescriptionResult::Empty,
        Some(_) => DescriptionResult::Present,
        None => DescriptionResult::Missing,
    }
}
