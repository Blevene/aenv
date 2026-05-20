//! Adapter TOML parsing and registry.
//!
//! An adapter declares a tool's project-relative paths and (in Phase 2)
//! merge strategies. Phase 1 supports parsing the minimal `name` + `files`
//! fields; merge strategies are accepted via serde's default but unused.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// A parsed adapter definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adapter {
    /// Adapter name (e.g. "claude-code").
    pub name: String,
    /// Project-relative paths or directory prefixes the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
    /// Merge strategies keyed by relative path. Unused in Phase 1.
    #[serde(default)]
    pub merge_strategies: BTreeMap<String, String>,
}

impl Adapter {
    /// Parse an adapter from a TOML string.
    pub fn from_toml(input: &str) -> Result<Self> {
        toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))
    }
}

/// In-memory set of loaded adapters, keyed by name.
#[derive(Debug, Default, Clone)]
pub struct AdapterRegistry {
    adapters: BTreeMap<String, Adapter>,
}

impl AdapterRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of adapters loaded.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Add an adapter to the registry.
    pub fn insert(&mut self, adapter: Adapter) {
        self.adapters.insert(adapter.name.clone(), adapter);
    }

    /// Look up an adapter by name.
    pub fn get(&self, name: &str) -> Option<&Adapter> {
        self.adapters.get(name)
    }

    /// Iterate over all adapters.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Adapter)> {
        self.adapters.iter()
    }

    /// Load every `.toml` file from `dir` into a registry. Non-TOML files
    /// are silently skipped. A missing `dir` returns an empty registry.
    pub fn load_from_dir<F: Filesystem>(fs: &F, dir: &Path) -> Result<Self> {
        let mut reg = Self::new();
        if !fs.exists(dir)? {
            return Ok(reg);
        }
        for path in fs.list_dir(dir)? {
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            let bytes = fs.read(&path)?;
            let toml_str = std::str::from_utf8(&bytes).map_err(|e| {
                AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", path.display()))
            })?;
            reg.insert(Adapter::from_toml(toml_str)?);
        }
        Ok(reg)
    }
}
