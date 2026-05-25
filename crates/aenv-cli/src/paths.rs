//! Path resolution for the CLI layer.
//!
//! `AENV_HOME` (env var, default `~/.aenv`) and `--project` (flag,
//! default ancestor-walk from cwd) are resolved here into absolute paths.
//! Library code below the CLI never reads env vars or `current_dir()`.

use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::project::find_project_root;
use std::path::PathBuf;

/// Resolve the registry root (`AENV_HOME`).
pub fn resolve_aenv_home() -> Result<PathBuf> {
    if let Ok(explicit) = std::env::var("AENV_HOME") {
        let p = PathBuf::from(explicit);
        if !p.is_absolute() {
            return Err(AenvError::ManifestInvalid(format!(
                "AENV_HOME must be absolute, got '{}'",
                p.display()
            )));
        }
        return Ok(p);
    }
    let home = std::env::var("HOME").map_err(|_| {
        AenvError::ManifestInvalid("HOME not set; cannot derive default AENV_HOME".to_string())
    })?;
    Ok(PathBuf::from(home).join(".aenv"))
}

/// Resolve the project root, given an optional `--project` override.
/// Walks ancestors from `cwd` looking for `.aenv` when no override.
pub fn resolve_project_root<F: Filesystem>(fs: &F, explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.is_absolute() {
            return Err(AenvError::ManifestInvalid(format!(
                "--project must be absolute, got '{}'",
                p.display()
            )));
        }
        return Ok(p);
    }
    let cwd = std::env::current_dir().map_err(AenvError::Io)?;
    find_project_root(fs, &cwd)
}

/// Resolve the project root for commands that CREATE a pin (`aenv use`).
/// Walks ancestors first — if an existing pin is found in an ancestor, the
/// command should overwrite that pin (the user is somewhere inside an
/// existing project tree). When no pin exists anywhere, fall back to cwd:
/// the user is establishing a new project here.
pub fn resolve_project_root_for_pin<F: Filesystem>(
    fs: &F,
    explicit: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.is_absolute() {
            return Err(AenvError::ManifestInvalid(format!(
                "--project must be absolute, got '{}'",
                p.display()
            )));
        }
        return Ok(p);
    }
    let cwd = std::env::current_dir().map_err(AenvError::Io)?;
    match find_project_root(fs, &cwd) {
        Ok(root) => Ok(root),
        Err(AenvError::ProjectNotPinned) => Ok(cwd),
        Err(other) => Err(other),
    }
}
