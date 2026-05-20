//! Namespace manifest (`aenv.toml`) parsing.
//!
//! Phase 1 consumes only `name`, `extends`, and `[adapters.<name>]` tables.
//! Forward-compat fields (`[parameters]`, `[policies]`, `[[skills]]`,
//! `[[agents]]`) are accepted but not yet parsed into typed values — they
//! land in Phases 3 and 4.

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A parsed namespace manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AenvManifest {
    /// Namespace name (must match the directory name; checked at activation time).
    pub name: String,

    /// Parent namespaces to inherit from. Empty in Phase 1; resolution lands in Phase 2.
    #[serde(default)]
    pub extends: Vec<String>,

    /// Per-adapter configuration. Keys are adapter names (e.g. "claude-code").
    #[serde(default)]
    pub adapters: BTreeMap<String, AdapterEntry>,
}

/// Per-adapter manifest entry: which files the adapter manages for this namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterEntry {
    /// Project-relative paths the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
}

impl AenvManifest {
    /// Parse a manifest from a TOML string. Returns `ManifestInvalid` on any
    /// parse error or missing required field.
    pub fn from_toml(input: &str) -> Result<Self> {
        let manifest: AenvManifest =
            toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;
        Ok(manifest)
    }

    /// Render the manifest to a canonical TOML string.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("AenvManifest serialization is infallible")
    }

    /// Build the manifest `aenv create <name>` writes by default — just the
    /// name, no adapters, no extends. Users add adapters by editing the file.
    pub fn default_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            extends: Vec::new(),
            adapters: BTreeMap::new(),
        }
    }
}
