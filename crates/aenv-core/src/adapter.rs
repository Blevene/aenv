//! Adapter TOML parsing and registry.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Allowed parameter type for an adapter declaration.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum AdapterParameterType {
    /// `parameter = "..."`
    #[serde(rename = "string")]
    String,
    /// `parameter = 1234`
    #[serde(rename = "integer")]
    Integer,
    /// `parameter = true`
    #[serde(rename = "boolean")]
    Boolean,
    /// `parameter = ["a", "b"]`
    #[serde(rename = "list-of-string")]
    ListString,
}

impl AdapterParameterType {
    /// String matching `ParameterValue::type_tag()`.
    pub fn type_tag(&self) -> &'static str {
        match self {
            AdapterParameterType::String => "string",
            AdapterParameterType::Integer => "integer",
            AdapterParameterType::Boolean => "boolean",
            AdapterParameterType::ListString => "list-of-string",
        }
    }
}

/// One parameter an adapter consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterParameterDecl {
    /// Parameter key (e.g. `"default_model"`).
    pub name: String,
    /// Expected TOML type.
    #[serde(rename = "type")]
    pub r#type: AdapterParameterType,
    /// Optional projection target. Phase 3 records this; Phase 4+ uses it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects_to: Option<String>,
}

/// A parsed adapter definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adapter {
    /// Adapter name (e.g. "claude-code").
    pub name: String,
    /// Project-relative paths or directory prefixes the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
    /// Phase 1 holdover — explicit per-file merge declaration on the adapter.
    #[serde(default)]
    pub merge_strategies: BTreeMap<String, String>,
    /// Per-path role declaration. Phase 2 understands `"instructions"`.
    #[serde(default)]
    pub roles: BTreeMap<String, String>,
    /// Per-path default merge strategy (consulted before role fallback).
    #[serde(default)]
    pub default_merge: BTreeMap<String, String>,
    /// Parameters this adapter consumes. Empty for adapters that take none.
    #[serde(default, rename = "parameters", skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<AdapterParameterDecl>,
    /// Adapter-specific directory under which skills are materialized in the
    /// project. Defaults to `None` (the adapter has no skill convention).
    /// For claude-code this is `.claude/skills`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_dir: Option<String>,
    /// Per-role character soft limits. Currently used only for the
    /// "instructions" role (R-24 / R-25). Empty for adapters that don't
    /// declare any role with a size guard.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub soft_limits: BTreeMap<String, usize>,
}

impl Adapter {
    /// Parse an adapter from a TOML string. Enforces no-duplicate parameter
    /// names within a single adapter.
    pub fn from_toml(input: &str) -> Result<Self> {
        let a: Adapter =
            toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for p in &a.parameters {
            if !seen.insert(p.name.as_str()) {
                return Err(AenvError::ManifestInvalid(format!(
                    "adapter '{}' declares parameter '{}' more than once",
                    a.name, p.name
                )));
            }
        }
        Ok(a)
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
