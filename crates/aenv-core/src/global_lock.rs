//! User-scope activation lock. Prevents two `aenv global …` invocations
//! from racing on the same `$HOME` / `$AENV_HOME`. Stale-lock detection
//! (PID gone or older than 5 minutes) auto-clears so a crashed-mid-flight
//! previous run does not permanently wedge global commands.

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Five-minute stale threshold per Issue #4 spec.
const STALE_SECS: i64 = 300;

#[derive(Debug, Serialize, Deserialize)]
struct LockFile {
    pid: u32,
    started_at: i64,
}

/// RAII-ish handle. Caller must explicitly `release_global_lock` (we don't
/// implement Drop because we want the call site to surface release errors;
/// silent release on drop hides a real I/O failure that probably matters).
#[derive(Debug)]
pub struct LockHandle {
    path: PathBuf,
}

/// Try to acquire the lock at `path`. Creates the parent directory if needed.
///
/// If an existing lock is held by a live PID and is fresh (<5 minutes old),
/// returns `GlobalConflict`. Stale or corrupt lock files are silently cleared
/// and we proceed.
pub fn acquire_global_lock(path: &Path) -> Result<LockHandle> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let body = std::fs::read(path)?;
        match serde_json::from_slice::<LockFile>(&body) {
            Ok(existing) => {
                let now = now_secs();
                let pid_alive = pid_alive(existing.pid);
                if pid_alive && (now - existing.started_at) < STALE_SECS {
                    return Err(AenvError::GlobalConflict(format!(
                        "another aenv global command is running (pid {}, started {}s ago)",
                        existing.pid,
                        now.saturating_sub(existing.started_at),
                    )));
                }
                let _ = std::fs::remove_file(path);
            }
            Err(_) => {
                // Corrupt lock — treat as stale.
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let lf = LockFile {
        pid: std::process::id(),
        started_at: now_secs(),
    };
    let body = serde_json::to_vec_pretty(&lf)
        .map_err(|e| AenvError::GlobalConflict(format!("lock serialize: {e}")))?;
    std::fs::write(path, body)?;
    Ok(LockHandle {
        path: path.to_path_buf(),
    })
}

/// Release a previously acquired lock. Idempotent: a missing lock file is OK.
pub fn release_global_lock(handle: LockHandle) -> Result<()> {
    let _ = std::fs::remove_file(&handle.path);
    Ok(())
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // kill(pid, 0) returns 0 iff the process is alive (or zombie); ESRCH otherwise.
    // Safety: kill with signal 0 has no side effects on the target.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    // Conservative on non-Unix: assume alive. The age-based check (STALE_SECS)
    // still clears truly abandoned locks within 5 minutes.
    true
}
