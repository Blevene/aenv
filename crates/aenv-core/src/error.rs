//! Public error type for `aenv-core`.
//!
//! Every variant maps to a documented exit code (PRD R-82). The CLI layer is
//! the only place that turns `AenvError` into an exit code; library callers
//! match on the variant.

use std::io;
use thiserror::Error;

/// All errors produced by `aenv-core`.
#[derive(Debug, Error)]
pub enum AenvError {
    /// Namespace name does not exist in the registry. Exit 10.
    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),

    /// Manifest names an adapter that is not installed. Exit 11.
    #[error("adapter not installed: {0}")]
    AdapterMissing(String),

    /// Manifest is malformed or contains an invalid value. Exit 12.
    #[error("manifest invalid: {0}")]
    ManifestInvalid(String),

    /// File materialization conflicts (e.g. atomicity probe failed). Exit 13.
    #[error("activation conflict: {0}")]
    ActivationConflict(String),

    /// Remote git operation failed. Exit 14.
    #[error("remote unreachable: {0}")]
    RemoteUnreachable(String),

    /// Cycle detected in `extends` chain. Exit 15.
    #[error("cycle in extends chain: {0}")]
    ExtendsCycle(String),

    /// `aenv get` named a parameter not declared by the resolution chain. Exit 16.
    #[error("parameter '{0}' is undefined in the resolution chain")]
    ParameterUndefined(String),

    /// Policy with `enforce = true` is violated. Exit 17.
    #[error("policy violation: {0}")]
    PolicyViolation(String),

    /// No `.aenv` pin and no `--project` flag. Exit 20.
    #[error("project not pinned")]
    ProjectNotPinned,

    /// I/O error from the underlying filesystem. Exit 1.
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

impl AenvError {
    /// Map this error to the documented exit code from PRD R-82.
    pub fn exit_code(&self) -> i32 {
        match self {
            AenvError::Io(_) => 1,
            AenvError::NamespaceNotFound(_) => 10,
            AenvError::AdapterMissing(_) => 11,
            AenvError::ManifestInvalid(_) => 12,
            AenvError::ActivationConflict(_) => 13,
            AenvError::RemoteUnreachable(_) => 14,
            AenvError::ExtendsCycle(_) => 15,
            AenvError::ParameterUndefined(_) => 16,
            AenvError::PolicyViolation(_) => 17,
            AenvError::ProjectNotPinned => 20,
        }
    }
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, AenvError>;
