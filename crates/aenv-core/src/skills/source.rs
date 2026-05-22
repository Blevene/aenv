//! Discriminate a skill `source` string by its form.
//!
//! Three shapes are recognized:
//!
//! * `/abs/path` or `~/path` → `Local`. Tilde expansion is the CLI's job.
//! * `git+<scheme>://...#<ref>` → `Git`. The `#<ref>` fragment is optional.
//! * `registry:<name>` → `Registry`. Phase 4 stubs resolution.
//!
//! Anything else is `ManifestInvalid` with a hint.

use crate::error::{AenvError, Result};
use std::path::PathBuf;

/// Parsed form of a skill source.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SourceKind {
    /// Filesystem path. May be absolute or tilde-prefixed.
    Local(PathBuf),
    /// Git URL (with `git+` prefix stripped) and optional `#ref`.
    Git {
        /// URL after the `git+` prefix, before any `#ref` fragment.
        url: String,
        /// Anything after a `#` fragment marker. `None` if not specified.
        ref_spec: Option<String>,
    },
    /// Forward-compat registry shorthand. Phase 4 stubs resolution.
    Registry(String),
}

impl SourceKind {
    /// Parse a source string into one of the three known shapes.
    pub fn parse(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(AenvError::ManifestInvalid(
                "skill source is empty".to_string(),
            ));
        }
        if let Some(rest) = s.strip_prefix("git+") {
            let (url, ref_spec) = match rest.split_once('#') {
                Some((u, r)) => (u.to_string(), Some(r.to_string())),
                None => (rest.to_string(), None),
            };
            return Ok(SourceKind::Git { url, ref_spec });
        }
        if let Some(name) = s.strip_prefix("registry:") {
            if name.is_empty() {
                return Err(AenvError::ManifestInvalid(
                    "registry source has empty name".to_string(),
                ));
            }
            return Ok(SourceKind::Registry(name.to_string()));
        }
        if s.starts_with('/') || s.starts_with('~') {
            return Ok(SourceKind::Local(PathBuf::from(s)));
        }
        Err(AenvError::ManifestInvalid(format!(
            "skill source '{s}' is not recognized as 'git+<url>[#ref]', \
             'registry:<name>', or an absolute / tilde-prefixed path. \
             Relative paths are rejected to avoid ambiguity."
        )))
    }
}
