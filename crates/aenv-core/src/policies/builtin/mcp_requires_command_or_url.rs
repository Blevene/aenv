//! `mcp_requires_command_or_url`: every entry under `mcpServers` (or `servers`)
//! in an MCP-role JSON file must declare `command` or `url`.

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::policies::builtin::{PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "mcp_requires_command_or_url";

/// Evaluate the policy. Looks for the `mcpServers` key, or `servers` as
/// alias. Per-server outcome is rolled up into a single Pass for the file
/// (if every server is fine) or one Warn/Fail per offending server.
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
        let adapter = match ctx.adapters.get(&c.adapter) {
            Some(a) => a,
            None => continue,
        };
        let role = adapter
            .roles
            .get(c.path.to_string_lossy().as_ref())
            .map(String::as_str)
            .unwrap_or("");
        if role != "mcp" {
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
        let v: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!("{} is not valid JSON: {e}", c.source_path.display()),
                ));
                continue;
            }
        };
        let servers = v
            .get("mcpServers")
            .or_else(|| v.get("servers"))
            .and_then(|x| x.as_object());
        let servers = match servers {
            Some(s) => s,
            None => {
                outcomes.push(PolicyOutcome::pass(KEY, Some(target.clone())));
                continue;
            }
        };
        let mut violations: Vec<String> = Vec::new();
        for (name, body) in servers {
            let ok = body
                .as_object()
                .map(|o| o.contains_key("command") || o.contains_key("url"))
                .unwrap_or(false);
            if !ok {
                violations.push(name.clone());
            }
        }
        if violations.is_empty() {
            outcomes.push(PolicyOutcome::pass(KEY, Some(target)));
        } else {
            let msg = format!(
                "{}: server(s) [{}] declare neither 'command' nor 'url'. \
                 Add one so the server can be reached.",
                c.path.display(),
                violations.join(", ")
            );
            outcomes.push(if policy.enforce {
                PolicyOutcome::fail(KEY, Some(target), msg)
            } else {
                PolicyOutcome::warn(KEY, Some(target), msg)
            });
        }
    }
    // Emit a targetless Pass when no MCP-role files matched, so `aenv doctor`
    // always shows a signal per evaluated policy.
    if outcomes.is_empty() {
        outcomes.push(PolicyOutcome::pass(KEY, None));
    }
    outcomes
}
