//! Schema for `aenv status --json`. Matches functional spec §7.1.

use crate::parameters::ResolvedParameter;
use crate::policies::ResolvedPolicy;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// JSON shape for `aenv status --json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StatusReport {
    /// Absolute path to the project root.
    pub project: PathBuf,
    /// Currently active namespace name, or `None` when inactive.
    pub active_namespace: Option<String>,
    /// Namespaces in the resolution chain, root → leaf.
    pub resolution_chain: Vec<String>,
    /// `sha256-v1:<hex>`. Present iff a namespace is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash: Option<String>,
    /// R-87 forward-compatibility hook: populated during the v2 dual-emit
    /// deprecation window. Always None in v1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash_v2: Option<String>,
    /// Effective parameters after `extends`-chain resolution.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ResolvedParameter>,
    /// Effective policies after `extends`-chain resolution.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub policies: BTreeMap<String, ResolvedPolicy>,
    /// Files currently managed by the active namespace.
    pub managed_files: Vec<ManagedFileJson>,
    /// Files that were backed up before activation.
    pub backed_up: Vec<BackedUpJson>,
    /// Non-fatal warnings produced during status resolution.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// JSON shape for a single managed file in `StatusReport`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ManagedFileJson {
    /// Project-relative path of the managed file.
    pub path: PathBuf,
    /// Qualified name (`<namespace>::<short-name>`). For deep-merged files
    /// whose contributors span multiple namespaces this is `(merged)::<path>`.
    pub qualified_name: String,
    /// Just the short-name portion, for adapter consumption per R-77.
    pub short_name: String,
    /// `null` for merged files with multi-namespace contributors.
    pub provided_by_namespace: Option<String>,
    /// One of `symlink`, `identical`, `copy`, `section-merge`,
    /// `deep-merge`, `merged` (legacy).
    pub strategy: String,
    /// For `deep-merge`: `json`, `yaml`, or `toml`. Omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_kind: Option<String>,
    /// Ordered chain of qualified names that contributed to a merged file.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<String>,
    /// Qualified names of artifacts that this artifact shadows.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<String>,
    /// Skill provenance for skill-managed files. `None` for regular files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_provenance: Option<SkillProvenanceJson>,
}

/// Skill provenance attached to skill-managed files.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SkillProvenanceJson {
    /// Source identifier: a local path, a git URL, or `"authored:<ns>"`.
    pub source: String,
    /// For git sources: the resolved commit SHA. `None` for local/authored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_ref: Option<String>,
    /// `"sha256:<hex>"` of the SKILL.md body at resolution time.
    pub resolved_hash: String,
}

/// A file that was backed up before activation.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BackedUpJson {
    /// Original project-relative path.
    pub path: PathBuf,
    /// Path where the backup was written.
    pub backup: PathBuf,
}
