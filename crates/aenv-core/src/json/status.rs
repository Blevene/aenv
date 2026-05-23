//! Schema for `aenv status --json`. Matches functional spec §7.1.

use crate::parameters::ResolvedParameter;
use crate::policies::ResolvedPolicy;
use crate::resolve::{DeepMergeFormat, MaterializeStrategy, ResolutionResult};
use crate::state::{ActivationState, ManagedFile};
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

impl StatusReport {
    /// Build a `StatusReport` from a project's `ActivationState` plus the
    /// freshly-computed resolution and hash. `hash` is the
    /// `sha256-v1:<hex>` string from `hash::hash_resolved_namespace`.
    pub fn build(
        project_root: PathBuf,
        state: &ActivationState,
        resolution: &ResolutionResult,
        hash: String,
    ) -> Self {
        StatusReport {
            project: project_root,
            active_namespace: Some(state.active_namespace.clone()),
            resolution_chain: resolution
                .chain
                .iter()
                .map(|n| n.as_str().to_string())
                .collect(),
            resolved_hash: Some(hash),
            resolved_hash_v2: None,
            parameters: resolution.parameters.clone(),
            policies: resolution.policies.clone(),
            managed_files: state
                .managed_files
                .iter()
                .map(ManagedFileJson::from)
                .collect(),
            backed_up: state
                .backed_up
                .iter()
                .map(|b| BackedUpJson {
                    path: b.original_path.clone(),
                    backup: b.backup_path.clone(),
                })
                .collect(),
            warnings: state.warnings.clone(),
        }
    }

    /// Build a `StatusReport` for a project that has no active namespace.
    pub fn unpinned(project_root: PathBuf) -> Self {
        StatusReport {
            project: project_root,
            ..Default::default()
        }
    }
}

impl From<&ManagedFile> for ManagedFileJson {
    fn from(mf: &ManagedFile) -> Self {
        let (strategy_str, merge_kind) = match mf.strategy {
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
        ManagedFileJson {
            path: mf.path.clone(),
            qualified_name: mf.qualified_name.to_string(),
            short_name: mf.qualified_name.short().as_str().to_string(),
            provided_by_namespace: provided_by,
            strategy: strategy_str.to_string(),
            merge_kind: merge_kind.map(str::to_string),
            contributors: mf.contributors.iter().map(ToString::to_string).collect(),
            shadows: mf.shadows.iter().map(ToString::to_string).collect(),
            skill_provenance: mf.skill_provenance.as_ref().map(|p| SkillProvenanceJson {
                source: p.source.clone(),
                resolved_ref: p.resolved_ref.clone(),
                resolved_hash: p.resolved_hash.clone(),
            }),
        }
    }
}
