//! Pre-flight safety: scan settings.json candidates for command-shaped path
//! references that point at files that don't exist on disk.
//!
//! Catches the "namespace declares hooks pointing at runtime/cli.py but
//! doesn't ship runtime/" lockout class before activation succeeds. The
//! failure mode this prevents: a fail-closed PreToolUse hook denies every
//! subsequent shell call when its referenced binary is missing, leaving the
//! user unable to run `aenv global deactivate` from inside Claude Code.
//!
//! The scanner walks each candidate whose target basename is `settings.json`,
//! parses it as JSON, and inspects a fixed set of JSON pointers where Claude
//! Code expects command strings (hooks, MCP servers, statusLine, …). For
//! each command string it extracts `argv[0]`, resolves `$HOME` and
//! `$AENV_TARGET_ROOT` against the activation target, and reports the path
//! when:
//!
//! * `argv[0]` looks like a file path (absolute, `~/`, or contains `/`), AND
//! * the path is not in the set of files being materialized this run, AND
//! * `fs.exists(...)` returns `false`.
//!
//! Anything else (bare binary names, inline shell expressions, env var
//! references that resolve to a materialized file) is intentionally
//! ignored — false positives erode trust in the warning faster than missed
//! detections.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::fs::Filesystem;
use crate::resolve::Candidate;

/// One pre-flight diagnostic: a command-shaped path that wouldn't resolve
/// on disk after activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightFinding {
    /// The settings.json file (post-materialization absolute path) that
    /// contains the unresolved reference.
    pub settings_path: PathBuf,
    /// What kind of reference it was (which JSON pointer family).
    pub kind: PreflightKind,
    /// The full command string as it appears in the JSON.
    pub command: String,
    /// The resolved-to-absolute path we checked and found missing.
    pub missing_path: PathBuf,
}

/// JSON-pointer family for a pre-flight finding. Used to render a stable
/// label like `hooks/PreToolUse` or `mcpServers/foo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreflightKind {
    /// A `hooks.<event>.[].hooks.[].command` reference.
    Hook {
        /// The hook event name (e.g. `PreToolUse`).
        event: String,
    },
    /// A `mcpServers.<name>.command` reference.
    McpServer {
        /// The MCP server name.
        name: String,
    },
    /// A `statusLine.command` reference.
    StatusLine,
}

impl PreflightKind {
    /// Stable label suitable for human-readable rendering.
    pub fn as_label(&self) -> String {
        match self {
            PreflightKind::Hook { event } => format!("hooks/{event}"),
            PreflightKind::McpServer { name } => format!("mcpServers/{name}"),
            PreflightKind::StatusLine => "statusLine".to_string(),
        }
    }
}

/// Scan every settings.json candidate for command-shaped paths that won't
/// resolve on disk after activation.
///
/// `target_root` is the activation target — `$HOME` for `Scope::User`, the
/// project root for `Scope::Project`. `$HOME` and `$AENV_TARGET_ROOT` in
/// command strings resolve against it.
///
/// Returns `Ok(Vec::new())` for malformed JSON; the scan is best-effort and
/// should never block on unrelated parser errors. Returns `Err` only for
/// genuine filesystem errors reading candidate sources.
pub fn preflight_settings_commands<F: Filesystem>(
    fs: &F,
    target_root: &Path,
    candidates: &[Candidate],
) -> Result<Vec<PreflightFinding>> {
    // Build the set of paths that will exist after activation. Findings whose
    // resolved missing_path lies in this set are suppressed.
    let materialized: std::collections::BTreeSet<PathBuf> = candidates
        .iter()
        .map(|c| target_root.join(&c.path))
        .collect();

    let mut findings = Vec::new();
    for candidate in candidates {
        if candidate
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .is_none_or(|n| n != "settings.json")
        {
            continue;
        }
        let bytes = fs.read(&candidate.source_path)?;
        let value: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => {
                // Swallow parse errors. The doctor / json validation path
                // will surface them elsewhere; pre-flight's job is hook-path
                // existence, not schema validation.
                continue;
            }
        };

        let target_settings_path = target_root.join(&candidate.path);

        let mut refs: Vec<(String, PreflightKind)> = Vec::new();
        extract_hooks(&value, &mut refs);
        extract_mcp_servers(&value, &mut refs);
        extract_status_line(&value, &mut refs);

        for (command, kind) in refs {
            let Some(argv0) = extract_argv0(&command) else {
                continue;
            };
            if !looks_like_path(&argv0) {
                continue;
            }
            let resolved = resolve_env(&argv0, target_root);
            if !resolved.is_absolute() {
                // After env-var resolution, anything that's still relative
                // (e.g. `./script.sh`) is unanchored. We don't know what cwd
                // the hook runner will use, so treat as not-checkable.
                continue;
            }
            if materialized.contains(&resolved) {
                continue;
            }
            if fs.exists(&resolved).unwrap_or(false) {
                continue;
            }
            findings.push(PreflightFinding {
                settings_path: target_settings_path.clone(),
                kind,
                command,
                missing_path: resolved,
            });
        }
    }
    Ok(findings)
}

/// Walk `hooks.<event>.[].hooks.[].command` and `hooks.<event>.[].command`
/// (both shapes appear in real Claude Code settings.json files).
fn extract_hooks(v: &serde_json::Value, out: &mut Vec<(String, PreflightKind)>) {
    let Some(hooks) = v.get("hooks").and_then(|h| h.as_object()) else {
        return;
    };
    for (event, entries) in hooks {
        let Some(entries) = entries.as_array() else {
            continue;
        };
        for entry in entries {
            // Shape A: { matcher?, hooks: [ { command, ... }, ... ] }
            if let Some(inner) = entry.get("hooks").and_then(|h| h.as_array()) {
                for h in inner {
                    if let Some(cmd) = h.get("command").and_then(|c| c.as_str()) {
                        out.push((
                            cmd.to_string(),
                            PreflightKind::Hook {
                                event: event.clone(),
                            },
                        ));
                    }
                }
            }
            // Shape B: { command, ... } directly.
            if let Some(cmd) = entry.get("command").and_then(|c| c.as_str()) {
                out.push((
                    cmd.to_string(),
                    PreflightKind::Hook {
                        event: event.clone(),
                    },
                ));
            }
        }
    }
}

/// Walk `mcpServers.<name>.command`.
fn extract_mcp_servers(v: &serde_json::Value, out: &mut Vec<(String, PreflightKind)>) {
    let Some(servers) = v.get("mcpServers").and_then(|s| s.as_object()) else {
        return;
    };
    for (name, server) in servers {
        if let Some(cmd) = server.get("command").and_then(|c| c.as_str()) {
            out.push((
                cmd.to_string(),
                PreflightKind::McpServer { name: name.clone() },
            ));
        }
    }
}

/// Walk `statusLine.command`.
fn extract_status_line(v: &serde_json::Value, out: &mut Vec<(String, PreflightKind)>) {
    let Some(status) = v.get("statusLine") else {
        return;
    };
    if let Some(cmd) = status.get("command").and_then(|c| c.as_str()) {
        out.push((cmd.to_string(), PreflightKind::StatusLine));
    }
}

/// Extract `argv[0]` from a command string with minimal shell-aware parsing.
///
/// Respects `"..."` and `'...'` quoting on the leading token. Returns `None`
/// for empty / whitespace-only strings.
fn extract_argv0(command: &str) -> Option<String> {
    let s = command.trim_start();
    if s.is_empty() {
        return None;
    }
    let mut chars = s.chars();
    let first = chars.next()?;
    if first == '"' || first == '\'' {
        let quote = first;
        let mut buf = String::new();
        for c in chars {
            if c == quote {
                return Some(buf);
            }
            buf.push(c);
        }
        // Unterminated quote — fall through to whitespace split, returning the
        // raw remainder so a malformed command still produces a guess.
        return Some(buf);
    }
    let mut buf = String::new();
    buf.push(first);
    for c in chars {
        if c.is_whitespace() {
            break;
        }
        buf.push(c);
    }
    Some(buf)
}

/// Whether `argv0` looks like a path the user expects on disk (vs. a bare
/// binary name resolved through `$PATH`, or an inline shell expression).
fn looks_like_path(argv0: &str) -> bool {
    if argv0.is_empty() {
        return false;
    }
    if argv0.starts_with('{') || argv0.starts_with('(') {
        return false;
    }
    if argv0.starts_with('/')
        || argv0.starts_with("./")
        || argv0.starts_with("../")
        || argv0.starts_with("~/")
        || argv0.starts_with('$')
    {
        return true;
    }
    argv0.contains('/')
}

/// Resolve `$HOME` / `${HOME}` / `$AENV_TARGET_ROOT` / `${AENV_TARGET_ROOT}`
/// against `target_root`, plus an `~/` prefix. Other `$VAR` references are
/// left as-is and the result probably won't resolve to an absolute path —
/// the caller skips those via the `looks_like_path` / absolute-only filter.
fn resolve_env(argv0: &str, target_root: &Path) -> PathBuf {
    let root_str = target_root.to_string_lossy();
    let mut s = argv0.to_string();
    for needle in [
        "${HOME}",
        "$HOME",
        "${AENV_TARGET_ROOT}",
        "$AENV_TARGET_ROOT",
    ] {
        if s.contains(needle) {
            s = s.replace(needle, &root_str);
        }
    }
    if let Some(stripped) = s.strip_prefix("~/") {
        s = format!("{}/{stripped}", root_str.trim_end_matches('/'));
    } else if s == "~" {
        s = root_str.to_string();
    }
    PathBuf::from(s)
}
