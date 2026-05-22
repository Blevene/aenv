//! Namespace manifest (`aenv.toml`) parsing.
//!
//! Phase 3 adds `[parameters]` and (Task 6) `[policies]`. Both tables go
//! through a two-stage parse: first into `toml::Value`, then each entry is
//! validated and converted into its typed shape. Type errors surface as
//! `ManifestInvalid` (exit 12).

use crate::error::{AenvError, Result};
use crate::parameters::ParameterValue;
use crate::policies::{policy_table_from_toml, PolicyDecl};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A parsed namespace manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AenvManifest {
    /// Namespace name (must match the directory name; checked at activation time).
    pub name: String,

    /// Parent namespaces to inherit from. Resolution lives in Phase 2's
    /// `resolve::resolve_namespace`.
    #[serde(default)]
    pub extends: Vec<String>,

    /// Per-adapter configuration. Keys are adapter names (e.g. "claude-code").
    #[serde(default)]
    pub adapters: BTreeMap<String, AdapterEntry>,

    /// Typed parameters. Always non-`None` after a successful `from_toml`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterValue>,

    /// Policy declarations. Keys are policy names; values hold the typed value
    /// and whether the policy is enforced.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub policies: BTreeMap<String, PolicyDecl>,
}

/// Per-adapter manifest entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterEntry {
    /// Project-relative paths the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
    /// Per-file merge override. Key is relative path; value is one of:
    /// "section", "deep", "last-wins", "symlink".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<std::collections::BTreeMap<String, String>>,
}

impl AenvManifest {
    /// Parse a manifest from a TOML string. Two-stage: serde does the
    /// structural parse; then `[parameters]` entries are validated via
    /// `ParameterValue::from_toml_value`.
    pub fn from_toml(input: &str) -> Result<Self> {
        // Stage 1: structural parse into a raw shape that holds parameters as
        // `toml::Value` so we can validate per-entry.
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            #[serde(default)]
            extends: Vec<String>,
            #[serde(default)]
            adapters: BTreeMap<String, AdapterEntry>,
            #[serde(default)]
            parameters: BTreeMap<String, toml::Value>,
            #[serde(default)]
            policies: BTreeMap<String, toml::Value>,
        }
        let raw: Raw =
            toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;

        // Stage 2: validate each parameter entry.
        let mut parameters: BTreeMap<String, ParameterValue> = BTreeMap::new();
        for (k, v) in &raw.parameters {
            let pv = ParameterValue::from_toml_value(v).map_err(|e| match e {
                AenvError::ManifestInvalid(reason) => {
                    AenvError::ManifestInvalid(format!("parameter '{k}': {reason}"))
                }
                other => other,
            })?;
            parameters.insert(k.clone(), pv);
        }

        // Stage 3: validate each policy entry.
        let policies = policy_table_from_toml(&raw.policies)?;

        Ok(AenvManifest {
            name: raw.name,
            extends: raw.extends,
            adapters: raw.adapters,
            parameters,
            policies,
        })
    }

    /// Render the manifest to a canonical TOML string.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("AenvManifest serialization is infallible")
    }

    /// Build the manifest `aenv create <name>` writes by default.
    pub fn default_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            extends: Vec::new(),
            adapters: BTreeMap::new(),
            parameters: BTreeMap::new(),
            policies: BTreeMap::new(),
        }
    }
}
