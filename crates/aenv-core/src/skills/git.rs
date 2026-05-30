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
            .is_ok_and(|o| o.status.success())
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
///
/// When `ref_spec` is a 40-char SHA, `git clone --branch <SHA>` is rejected
/// by git (`fatal: Remote branch <SHA> not found in upstream origin`).
/// We fall back to `git init` + `git fetch --depth 1 <url> <sha>` +
/// `git checkout FETCH_HEAD` for SHA-shaped refs. This requires the remote
/// to allow fetching arbitrary commits (`uploadpack.allowReachableSHA1InWant`),
/// which is true for github.com and most self-hosted forges.
/// `sub_path`, when `Some`, restricts the clone to that subdirectory via a
/// cone-mode sparse checkout plus a `--filter=blob:none` partial clone — so
/// importing one skill out of a large monorepo (e.g.
/// `microsoft/ai-agents-for-beginners`) fetches a few KB instead of the whole
/// tree. `None` does a full shallow clone (whole-repo sources).
pub fn git_clone(
    url: &str,
    ref_spec: Option<&str>,
    dest: &Path,
    sub_path: Option<&str>,
) -> Result<String> {
    if !git_available() {
        return Err(AenvError::RemoteUnreachable("git not on PATH".to_string()));
    }
    if let Some(r) = ref_spec {
        if is_full_sha(r) {
            return clone_by_sha(url, r, dest, sub_path);
        }
    }
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg("--depth").arg("1");
    if sub_path.is_some() {
        // Defer checkout until the sparse set is configured; partial-clone so
        // blobs outside the sparse cone aren't downloaded.
        cmd.arg("--no-checkout").arg("--filter=blob:none");
    }
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
    if let Some(p) = sub_path {
        run_git_in(dest, &["sparse-checkout", "set", "--cone", p], url)?;
        run_git_in(dest, &["checkout"], url)?;
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

/// True if `s` is a full-length (40 char) lowercase hexadecimal commit SHA.
fn is_full_sha(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Init+fetch+checkout for fetching a specific commit SHA from a remote.
/// Used because `git clone --branch <SHA>` is not supported. When `sub_path`
/// is `Some`, configures a cone-mode sparse checkout and a `blob:none` partial
/// fetch so only that subtree is materialized.
fn clone_by_sha(url: &str, sha: &str, dest: &Path, sub_path: Option<&str>) -> Result<String> {
    std::fs::create_dir_all(dest).map_err(|e| {
        AenvError::RemoteUnreachable(format!("create clone dest {}: {e}", dest.display()))
    })?;
    run_git_in(dest, &["init", "--quiet"], url)?;
    run_git_in(dest, &["remote", "add", "origin", url], url)?;
    if let Some(p) = sub_path {
        run_git_in(dest, &["sparse-checkout", "set", "--cone", p], url)?;
        run_git_in(
            dest,
            &["fetch", "--depth", "1", "--filter=blob:none", "origin", sha],
            url,
        )?;
    } else {
        run_git_in(dest, &["fetch", "--depth", "1", "origin", sha], url)?;
    }
    run_git_in(
        dest,
        &["-c", "advice.detachedHead=false", "checkout", "FETCH_HEAD"],
        url,
    )?;
    Ok(sha.to_string())
}

/// Ensure `sub_path` is materialized in an existing cache clone. If the clone
/// is sparse (created by a prior `--path` import), add the path to the cone so
/// multiple skills from the same repo+ref accumulate without re-cloning. If the
/// clone is full (a legacy whole-repo clone), this is a no-op — every path is
/// already present, and we must NOT sparse-ify it (that would hide the paths
/// other already-imported skills depend on).
pub fn ensure_sparse_path(dest: &Path, sub_path: &str) -> Result<()> {
    if !git_available() {
        return Ok(());
    }
    if !dest.join(".git/info/sparse-checkout").exists() {
        // Not a sparse clone — full tree already on disk; leave it alone.
        return Ok(());
    }
    run_git_in(dest, &["sparse-checkout", "add", sub_path], "<cache>")
}

/// Run a `git <args>` invocation inside `dir`, returning `RemoteUnreachable`
/// on failure. `url` is included in the error for diagnostics.
fn run_git_in(dir: &Path, args: &[&str], url: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| {
            AenvError::RemoteUnreachable(format!("git {} ({url}): {e}", args.join(" ")))
        })?;
    if !output.status.success() {
        return Err(AenvError::RemoteUnreachable(format!(
            "git {} ({url}) failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}
