//! Activation state file (`.aenv-state/state.json`).
//!
//! Persisted after a successful activation. Records the active namespace,
//! every file aenv materialized, every original it backed up, and a schema
//! version so older binaries can refuse to operate on newer state files
//! (engineering §11).

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current schema version. Bump when changing the on-disk shape.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Materialization strategy used for a single managed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterializeStrategy {
    /// File is a symlink into the namespace directory.
    Symlink,
    /// File is a copy of the namespace file (Windows fallback, Phase 7).
    Copy,
    /// Project file's bytes match the namespace's, so aenv left it in
    /// place rather than symlinking over it. At deactivate time we
    /// likewise leave it alone: it's the user's content (and also the
    /// namespace's), so removing it would surprise the user.
    Identical,
    /// File is a merged regular file (Phase 2).
    Merged,
}

/// One file managed by the current activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedFile {
    /// Project-relative path.
    pub path: PathBuf,
    /// How the file was materialized.
    pub strategy: MaterializeStrategy,
    /// Source path inside the registry (None for `Identical`/`Merged`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source: Option<PathBuf>,
}

/// A file aenv backed up before materializing on top of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackedUpFile {
    /// Project-relative path of the original.
    pub original_path: PathBuf,
    /// Project-relative path of the backup copy.
    pub backup_path: PathBuf,
}

/// Persisted state of an active namespace in a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationState {
    /// Schema version of this file.
    pub schema_version: u32,
    /// Name of the active namespace.
    pub active_namespace: String,
    /// Absolute path to the project root.
    pub project_root: PathBuf,
    /// Files this activation materialized.
    pub managed_files: Vec<ManagedFile>,
    /// Files this activation backed up before materializing over them.
    pub backed_up: Vec<BackedUpFile>,
}

impl ActivationState {
    /// Serialize to pretty JSON for on-disk storage.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AenvError::ManifestInvalid(format!("state serialization: {e}")))
    }

    /// Deserialize from JSON, rejecting any unknown future schema version.
    pub fn from_json(input: &str) -> Result<Self> {
        let state: ActivationState = serde_json::from_str(input)
            .map_err(|e| AenvError::ManifestInvalid(format!("state parse: {e}")))?;
        if state.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(AenvError::ManifestInvalid(format!(
                "state schema_version {} > supported {}; upgrade aenv",
                state.schema_version, CURRENT_SCHEMA_VERSION
            )));
        }
        Ok(state)
    }
}
