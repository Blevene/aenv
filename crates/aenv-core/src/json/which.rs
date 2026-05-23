//! Schema for `aenv which --json`.

use crate::resolve::{DeepMergeFormat, MaterializeStrategy};
use crate::state::ManagedFile;
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

impl WhichReport {
    /// Build a `WhichReport` from a `ManagedFile` (the per-file entry
    /// recorded in `.aenv-state/state.json`).
    pub fn from_managed_file(mf: &ManagedFile) -> Self {
        let (strategy, merge_kind) = match mf.strategy {
            MaterializeStrategy::Symlink => ("symlink", None),
            MaterializeStrategy::Identical => ("identical", None),
            MaterializeStrategy::Copy => ("copy", None),
            MaterializeStrategy::SectionMerge => ("section-merge", None),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Json) => ("deep-merge", Some("json")),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Yaml) => ("deep-merge", Some("yaml")),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Toml) => ("deep-merge", Some("toml")),
            MaterializeStrategy::Merged => ("merged", None),
        };
        let provided_by = if mf.qualified_name.namespace().as_str()
            == crate::identity::NamespaceId::RESERVED_MERGED
        {
            None
        } else {
            Some(mf.qualified_name.namespace().as_str().to_string())
        };
        WhichReport {
            path: mf.path.clone(),
            qualified_name: mf.qualified_name.to_string(),
            short_name: mf.qualified_name.short().as_str().to_string(),
            provided_by_namespace: provided_by,
            strategy: strategy.to_string(),
            merge_kind: merge_kind.map(str::to_string),
            contributors: mf.contributors.iter().map(ToString::to_string).collect(),
            shadows: mf.shadows.iter().map(ToString::to_string).collect(),
        }
    }
}
