//! Shell-out wrapper around system `git`.
//!
//! Used by the imported-skill resolver. Tests should gate on `git_available()`
//! so they skip cleanly when git isn't on PATH.
//!
//! Why shell out rather than use libgit2: `git2`'s dependency footprint is
//! large (libgit2 + libssh2 + zlib + libssl), and `aenv` only needs three
//! operations (ls-remote, clone --depth 1, rev-parse HEAD). The shell-out
//! is small, well-understood, and inherits the user's git config (auth,
//! credential helpers, proxy).

use crate::error::{AenvError, Result};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

static GIT_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Return whether `git --version` succeeds. Result is cached for the process.
pub fn git_available() -> bool {
    *GIT_AVAILABLE.get_or_init(|| {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Resolve a (url, ref_spec) pair to a commit SHA via `git ls-remote`.
/// When `ref_spec` is `None`, returns the SHA for HEAD.
pub fn git_resolve_ref(url: &str, ref_spec: Option<&str>) -> Result<String> {
    if !git_available() {
        return Err(AenvError::RemoteUnreachable("git not on PATH".to_string()));
    }
    let mut cmd = Command::new("git");
    cmd.arg("ls-remote").arg(url);
    if let Some(r) = ref_spec {
        cmd.arg(r);
    }
    let output = cmd
        .output()
        .map_err(|e| AenvError::RemoteUnreachable(format!("git ls-remote {url}: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AenvError::RemoteUnreachable(format!(
            "git ls-remote {url} failed: {}",
            stderr.trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // First field of the first non-empty line is the SHA.
    let sha = stdout
        .lines()
        .find_map(|line| line.split_whitespace().next())
        .ok_or_else(|| {
            AenvError::RemoteUnreachable(format!("git ls-remote {url} returned no matching refs"))
        })?;
    Ok(sha.to_string())
}

/// Shallow-clone `url` at `ref_spec` (or HEAD) into `dest`. Returns the
/// resolved commit SHA. `dest` must not exist (git will create it).
pub fn git_clone(url: &str, ref_spec: Option<&str>, dest: &Path) -> Result<String> {
    if !git_available() {
        return Err(AenvError::RemoteUnreachable("git not on PATH".to_string()));
    }
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg("--depth").arg("1");
    if let Some(r) = ref_spec {
        cmd.arg("--branch").arg(r);
    }
    cmd.arg(url).arg(dest);
    let output = cmd
        .output()
        .map_err(|e| AenvError::RemoteUnreachable(format!("git clone {url}: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AenvError::RemoteUnreachable(format!(
            "git clone {url} failed: {}",
            stderr.trim()
        )));
    }
    // Resolve the actual HEAD commit in the clone.
    let head = Command::new("git")
        .current_dir(dest)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| AenvError::RemoteUnreachable(format!("git rev-parse: {e}")))?;
    if !head.status.success() {
        let stderr = String::from_utf8_lossy(&head.stderr);
        return Err(AenvError::RemoteUnreachable(format!(
            "git rev-parse HEAD failed: {}",
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&head.stdout).trim().to_string())
}
