//! Namespace-scoped lifecycle-script approval.
//!
//! The user explicitly opts a namespace's `on_activate` script into "run me
//! on every activation"; the approval is invalidated when the script's
//! bytes change so a re-prompt is required if the script is edited or
//! replaced. The approval marker lives under the namespace's own directory
//! (`<aenv_home>/envs/<ns>/.approved`) so deleting a namespace also clears
//! its consent record.

use aenv_core::error::{AenvError, Result};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use sha2::{Digest, Sha256};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// Outcome of comparing a namespace's `.approved` marker against the
/// current `on_activate` script's bytes.
pub enum ApprovalStatus {
    /// No `[lifecycle].on_activate` declared — no prompt needed at all.
    NoScript,
    /// Marker exists and matches the current script's sha256.
    Approved,
    /// Marker exists but the script's sha256 has changed since approval.
    ScriptChanged {
        previous_sha: String,
        current_sha: String,
    },
    /// No marker file: first-time approval flow.
    NotApproved { current_sha: String },
}

/// The `.approved` marker path for `ns` under `layout`.
pub fn marker_path(layout: &RegistryLayout, ns: &NamespaceId) -> PathBuf {
    layout.namespace_dir(ns.as_str()).join(".approved")
}

/// Compute the `sha256:<hex>` digest of the file at `script_path`.
pub fn script_sha(script_path: &Path) -> Result<String> {
    let bytes = std::fs::read(script_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(format!("sha256:{}", hex_lower(&digest)))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Inspect the namespace's current approval status for `script_path`. Pass
/// `None` when the namespace has no `on_activate` declared at all.
pub fn current_status(
    layout: &RegistryLayout,
    ns: &NamespaceId,
    script_path: Option<&Path>,
) -> Result<ApprovalStatus> {
    let script_path = match script_path {
        Some(p) => p,
        None => return Ok(ApprovalStatus::NoScript),
    };
    let current_sha = script_sha(script_path)?;
    let marker = marker_path(layout, ns);
    if !marker.exists() {
        return Ok(ApprovalStatus::NotApproved { current_sha });
    }
    let prev = std::fs::read_to_string(&marker)?;
    let prev = prev.trim().to_string();
    if prev == current_sha {
        Ok(ApprovalStatus::Approved)
    } else {
        Ok(ApprovalStatus::ScriptChanged {
            previous_sha: prev,
            current_sha,
        })
    }
}

/// Persist `sha` as the current approval for `ns`. Creates the namespace
/// directory if it does not exist (it always should at this point, but we
/// defend against a not-yet-materialized namespace anyway).
pub fn record_approval(layout: &RegistryLayout, ns: &NamespaceId, sha: &str) -> Result<()> {
    let marker = marker_path(layout, ns);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&marker, format!("{sha}\n"))?;
    Ok(())
}

/// Render the prompt to stdout and read a y/n answer from stdin. Returns
/// `true` if the user approved, `false` for any other response (including
/// a closed stdin — non-interactive runs decline by default rather than
/// hang). The wording diverges between the first-time and re-approval
/// branches via `change_from`.
pub fn prompt_user(script_path: &Path, sha: &str, change_from: Option<&str>) -> Result<bool> {
    let bytes = std::fs::read(script_path)?;
    let body = String::from_utf8_lossy(&bytes);
    let preview: String = body.lines().take(8).map(|l| format!("    {l}\n")).collect();

    if let Some(prev) = change_from {
        println!();
        println!("The on_activate script has changed since your last approval:");
        println!("  Script: {}", script_path.display());
        println!("  Previously approved: {prev}");
        println!("  Current:             {sha}");
        println!("  First 8 lines:");
        print!("{preview}");
        print!("Re-approve? [y/N]: ");
    } else {
        println!();
        println!("About to run on_activate hook:");
        println!("  Script: {}", script_path.display());
        println!("  sha256: {sha}");
        println!("  First 8 lines:");
        print!("{preview}");
        print!("Allow this script to run on every future activation until its content changes? [y/N]: ");
    }
    io::stdout()
        .flush()
        .map_err(|e| AenvError::ManifestInvalid(format!("stdout flush: {e}")))?;

    let stdin = io::stdin();
    let mut line = String::new();
    // Read a single line; EOF on a closed stdin returns 0 bytes and leaves
    // `line` empty, which counts as "no".
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| AenvError::ManifestInvalid(format!("stdin read: {e}")))?;
    let answer = line.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}
