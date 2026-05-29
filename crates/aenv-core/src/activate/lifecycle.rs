//! Lifecycle script execution (`[lifecycle].on_activate` / `on_deactivate`).
//!
//! Scripts run with:
//!
//! - `cwd` = the activation `target_root` (project root or `$HOME`)
//! - `AENV_NAMESPACE` = leaf namespace name
//! - `AENV_SCOPE` = `"project"` or `"user"`
//! - `AENV_TARGET_ROOT` = absolute target_root path
//! - `AENV_NAMESPACE_DIR` = absolute namespace dir under the registry
//! - `AENV_LIFECYCLE_EVENT` = `"activate"` or `"deactivate"`
//! - `AENV_FORCE=1` if the user passed `--force` (deactivate path)
//!
//! `stdout` / `stderr` are inherited so the user sees `pip install` output,
//! brew progress, etc. directly. Exit-status semantics differ between
//! activate and deactivate (see callers in `activate/mod.rs` /
//! `deactivate.rs`).

use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use std::path::Path;
use std::process::Command;

/// Which lifecycle boundary is running. Affects the `AENV_LIFECYCLE_EVENT`
/// env var the script sees; callers handle exit-status semantics
/// (activate rolls back on failure; deactivate warns and continues).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleEvent {
    Activate,
    Deactivate,
}

impl LifecycleEvent {
    fn as_str(self) -> &'static str {
        match self {
            LifecycleEvent::Activate => "activate",
            LifecycleEvent::Deactivate => "deactivate",
        }
    }
}

/// Run a lifecycle script. Returns `Ok(())` on a zero exit; any other outcome
/// becomes `io::Error` with the exit status in the message.
///
/// `force` propagates the user's `--force` flag (only meaningful for
/// deactivate today — activate ignores `--force` semantics for now).
pub(crate) fn run_lifecycle_script(
    script_path: &Path,
    target_root: &Path,
    layout: &RegistryLayout,
    leaf: &NamespaceId,
    scope: crate::scope::Scope,
    event: LifecycleEvent,
    force: bool,
) -> std::io::Result<()> {
    // aenv copies lifecycle scripts into the namespace dir, and the import /
    // snapshot copy path writes bytes only — it drops the source file's
    // executable bit. Since we exec the script directly (honoring its
    // shebang), restore owner-execute first so a locally- or git-imported
    // `on_activate` isn't refused with "Permission denied". No-op when the
    // bit is already set.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(script_path) {
            let mode = meta.permissions().mode();
            if mode & 0o111 == 0 {
                let mut perms = meta.permissions();
                perms.set_mode(mode | 0o100);
                let _ = std::fs::set_permissions(script_path, perms);
            }
        }
    }

    let mut cmd = Command::new(script_path);
    cmd.current_dir(target_root);
    cmd.env("AENV_NAMESPACE", leaf.as_str());
    cmd.env("AENV_SCOPE", scope.as_str());
    cmd.env("AENV_TARGET_ROOT", target_root);
    cmd.env("AENV_NAMESPACE_DIR", layout.namespace_dir(leaf.as_str()));
    cmd.env("AENV_LIFECYCLE_EVENT", event.as_str());
    if force {
        cmd.env("AENV_FORCE", "1");
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "lifecycle script exited with {status}"
        )));
    }
    Ok(())
}
