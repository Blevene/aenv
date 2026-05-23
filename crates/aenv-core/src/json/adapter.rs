//! Schema for `aenv adapter list --json`.

use serde::Serialize;

/// JSON shape for one adapter entry in `aenv adapter list --json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AdapterEntryJson {
    /// Adapter name (e.g. `claude-code`).
    pub name: String,
    /// Files the adapter manages (short names).
    pub files: Vec<String>,
    /// Directory the adapter reads skills from, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_dir: Option<String>,
    /// Parameters this adapter declares.
    pub parameters: Vec<AdapterParameterJson>,
    /// Soft resource limits keyed by limit name.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub soft_limits: std::collections::BTreeMap<String, usize>,
}

/// One parameter declared by an adapter.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AdapterParameterJson {
    /// Parameter name.
    pub name: String,
    /// `string`, `integer`, `boolean`, `list-of-string`.
    #[serde(rename = "type")]
    pub type_: String,
    /// Target file path the parameter is projected into, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projects_to: Option<String>,
}
