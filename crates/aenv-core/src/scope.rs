//! Activation scope: project-local (`<project>/`) vs user-global (`$HOME/`).
//!
//! Every materialization primitive that previously took a `project_root: &Path`
//! is now parameterized by a scope. The scope determines both the target root
//! (where files land) and the filter applied to namespace manifests (which
//! adapter file list is consulted: `files` vs `user_files`).

use serde::{Deserialize, Serialize};

/// Activation scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Project-local activation: files materialize under `<project_root>/`.
    /// State at `<project_root>/.aenv-state/state.json`.
    #[default]
    Project,
    /// User-global activation: files materialize under `$HOME/`.
    /// State at `$AENV_HOME/global-state.json`.
    User,
}

impl Scope {
    /// Stable lowercase identifier for diagnostics and JSON output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Project => "project",
            Scope::User => "user",
        }
    }
}
