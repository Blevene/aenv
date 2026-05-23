//! Schema for `aenv which --json`.

use serde::Serialize;
use std::path::PathBuf;

/// JSON shape for `aenv which <file> --json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WhichReport {
    /// Resolved absolute path of the managed file.
    pub path: PathBuf,
    /// Qualified name (`<namespace>::<short-name>`).
    pub qualified_name: String,
    /// Short name portion only.
    pub short_name: String,
    /// Namespace that owns the file. `None` for multi-contributor merged files.
    pub provided_by_namespace: Option<String>,
    /// Materialisation strategy: `symlink`, `copy`, `section-merge`, etc.
    pub strategy: String,
    /// For `deep-merge`: `json`, `yaml`, or `toml`. Omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_kind: Option<String>,
    /// Ordered chain of qualified-name contributors (for merged files).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<String>,
    /// Qualified names that this file shadows.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<String>,
}
