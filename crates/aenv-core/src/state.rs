//! Activation state file (`.aenv-state/state.json`).
//!
//! Persisted after a successful activation. Records the active namespace,
//! every file aenv materialized, every original it backed up, and a schema
//! version so older binaries can refuse to operate on newer state files
//! (engineering §11).

use crate::error::{AenvError, Result};
use std::path::PathBuf;

/// Current schema version. Bump when changing the on-disk shape.
pub const SCHEMA_VERSION: u32 = 4;

/// Materialization strategy — re-exported from `crate::resolve` so callers
/// only need one import path.
pub use crate::resolve::MaterializeStrategy;

/// Provenance record for a skill-managed file.
///
/// Attached to the SKILL.md of imported skills so `aenv status` can display
/// the origin and hash without re-reading the source.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillProvenance {
    /// Source identifier: a local path, a git URL, or `"authored:<ns>"`.
    pub source: String,
    /// For git sources: the resolved commit SHA. `None` for local/authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_ref: Option<String>,
    /// `"sha256:<hex>"` of the SKILL.md body at resolution time.
    pub resolved_hash: String,
}

/// One file managed by the current activation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ManagedFile {
    /// Project-relative path.
    pub path: PathBuf,
    /// Qualified name of the winning artifact (namespace::short-name).
    pub qualified_name: crate::identity::QualifiedName,
    /// How the file was materialized.
    pub strategy: crate::resolve::MaterializeStrategy,
    /// Ordered chain-of-contribution for merged artifacts. Empty otherwise.
    #[serde(default)]
    pub contributors: Vec<crate::identity::QualifiedName>,
    /// Earlier-chain qualified names that this artifact shadows. Empty for
    /// merged artifacts.
    #[serde(default)]
    pub shadows: Vec<crate::identity::QualifiedName>,
    /// Skill provenance for skill SKILL.md files. `None` for regular files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_provenance: Option<SkillProvenance>,
}

impl<'de> serde::Deserialize<'de> for ManagedFile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Raw {
            path: PathBuf,
            #[serde(default)]
            qualified_name: Option<crate::identity::QualifiedName>,
            strategy: crate::resolve::MaterializeStrategy,
            #[serde(default)]
            contributors: Vec<crate::identity::QualifiedName>,
            #[serde(default)]
            shadows: Vec<crate::identity::QualifiedName>,
            #[serde(default)]
            skill_provenance: Option<SkillProvenance>,
        }
        let raw = Raw::deserialize(d)?;
        // Absence of `qualified_name` means this is a schema-1 file. We use a
        // sentinel namespace so `ActivationState`'s custom deserializer can
        // patch it up with the real active_namespace once that is known.
        let qualified_name = raw.qualified_name.unwrap_or_else(|| {
            crate::identity::QualifiedName::new(
                crate::identity::NamespaceId::new("__schema_1__").expect("static"),
                crate::identity::ShortName::new(raw.path.to_string_lossy().to_string())
                    .expect("path validated upstream"),
            )
        });
        Ok(ManagedFile {
            path: raw.path,
            qualified_name,
            strategy: raw.strategy,
            contributors: raw.contributors,
            shadows: raw.shadows,
            skill_provenance: raw.skill_provenance,
        })
    }
}

/// A file aenv backed up before materializing on top of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackedUpFile {
    /// Project-relative path of the original.
    pub original_path: PathBuf,
    /// Project-relative path of the backup copy.
    pub backup_path: PathBuf,
}

/// Persisted state of an active namespace in a project.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ActivationState {
    /// Schema version of this file.
    pub schema_version: u32,
    /// Name of the active namespace.
    pub active_namespace: String,
    /// Absolute path to the project root.
    pub project_root: PathBuf,
    /// Files this activation materialized.
    pub managed_files: Vec<ManagedFile>,
    /// Files this activation backed up before materializing over them.
    pub backed_up: Vec<BackedUpFile>,
    /// Effective parameters after `extends` resolution (Phase 3).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub parameters: std::collections::BTreeMap<String, crate::parameters::ResolvedParameter>,
    /// Effective policies after `extends` resolution (Phase 3).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub policies: std::collections::BTreeMap<String, crate::policies::ResolvedPolicy>,
}

impl<'de> serde::Deserialize<'de> for ActivationState {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        use std::collections::BTreeMap;
        #[derive(serde::Deserialize)]
        struct Raw {
            schema_version: u32,
            active_namespace: String,
            project_root: PathBuf,
            #[serde(default)]
            managed_files: Vec<ManagedFile>,
            #[serde(default)]
            backed_up: Vec<BackedUpFile>,
            #[serde(default)]
            parameters: BTreeMap<String, crate::parameters::ResolvedParameter>,
            #[serde(default)]
            policies: BTreeMap<String, crate::policies::ResolvedPolicy>,
        }
        let mut raw = Raw::deserialize(d)?;
        // For schema-1 files the ManagedFile deserializer used a sentinel
        // namespace. Patch those entries with the real active namespace now
        // that we know it.
        if raw.schema_version == 1 {
            let ns = crate::identity::NamespaceId::new(raw.active_namespace.as_str())
                .map_err(serde::de::Error::custom)?;
            for mf in &mut raw.managed_files {
                if mf.qualified_name.namespace().as_str() == "__schema_1__" {
                    mf.qualified_name = crate::identity::QualifiedName::new(
                        ns.clone(),
                        mf.qualified_name.short().clone(),
                    );
                }
            }
        }
        Ok(ActivationState {
            schema_version: raw.schema_version,
            active_namespace: raw.active_namespace,
            project_root: raw.project_root,
            managed_files: raw.managed_files,
            backed_up: raw.backed_up,
            parameters: raw.parameters,
            policies: raw.policies,
        })
    }
}

impl ActivationState {
    /// Serialize to pretty JSON for on-disk storage.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AenvError::ManifestInvalid(format!("state serialization: {e}")))
    }

    /// Deserialize from JSON, rejecting any unknown future schema version.
    pub fn from_json(input: &str) -> Result<Self> {
        let state: ActivationState = serde_json::from_str(input)
            .map_err(|e| AenvError::ManifestInvalid(format!("state parse: {e}")))?;
        if state.schema_version > SCHEMA_VERSION {
            return Err(AenvError::ManifestInvalid(format!(
                "state schema_version {} > supported {}; upgrade aenv",
                state.schema_version, SCHEMA_VERSION
            )));
        }
        Ok(state)
    }
}
