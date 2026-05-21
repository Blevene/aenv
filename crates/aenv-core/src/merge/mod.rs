//! Merge algorithms for composed namespaces.
//!
//! Each submodule owns one strategy:
//!   * `section`  — Markdown by `#`/`##` section, with `<!-- aenv:replace -->`
//!   * `deep_json`, `deep_yaml`, `deep_toml` — structured deep-merge per format (added in Tasks 6-8)
//!
//! All merge functions take `Vec<bytes>` in chain order (root first) and
//! return the merged byte output. Errors are reported as `MergeError`.

pub mod deep_json;
pub mod deep_yaml;
pub mod section;

/// Errors produced by the merge algorithms.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    /// A parse error occurred while processing a file of the given kind.
    #[error("parse error in {kind}: {detail}")]
    Parse {
        /// File kind (e.g. `"json"`, `"yaml"`).
        kind: &'static str,
        /// Human-readable description of the parse failure.
        detail: String,
    },
    /// Two namespaces contributed incompatible types at the same path.
    #[error("incompatible types during {kind} merge at {path}")]
    TypeMismatch {
        /// File kind.
        kind: &'static str,
        /// Dotted path where the mismatch was detected.
        path: String,
    },
    /// A byte slice could not be decoded as UTF-8.
    #[error("UTF-8 decoding failed: {0}")]
    Utf8(String),
}

impl From<MergeError> for crate::AenvError {
    fn from(value: MergeError) -> Self {
        match value {
            MergeError::Parse { .. } => crate::AenvError::ManifestInvalid(value.to_string()),
            MergeError::TypeMismatch { .. } | MergeError::Utf8(_) => {
                crate::AenvError::ActivationConflict(value.to_string())
            }
        }
    }
}
