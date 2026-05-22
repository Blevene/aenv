//! Skill content model for namespaces.
//!
//! Phase 4 introduces two flavors of skill: *authored* skills whose files
//! live under the namespace's own directory (and materialize through the
//! standard adapter-files path), and *imported* skills whose `source` is
//! resolved at activation time (local path or git URL, optionally pinned).
//! `SourceKind` parsing lives in `skills::source` (Task 2); the `SkillDecl`
//! struct here is the wire shape that lands in `aenv.toml`'s `[[skills]]`
//! table.

pub mod cache;
pub mod git;
pub mod git_source;
pub mod local;
pub mod registry;
pub mod source;

use serde::{Deserialize, Serialize};

/// One `[[skills]]` entry in a manifest.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillDecl {
    /// Skill name. Becomes the directory name under the adapter's `skills_dir`
    /// at materialization time. Must be unique within a namespace.
    pub name: String,
    /// Whether the skill's files live in the namespace tree (`Authored`) or
    /// are fetched at activation time from a `source` (`Imported`).
    pub mode: SkillMode,
    /// Which adapter manages this skill. Optional when the namespace declares
    /// exactly one adapter (then defaults to that adapter at resolution time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// Required for `mode = "imported"`. The form of the source determines
    /// how it's resolved (see `SourceKind` in `skills::source`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Optional pinned ref for `mode = "imported"`. When omitted, the
    /// importer resolves to head at each activation and records the resolved
    /// ref in `state.json`.
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    /// When `true`, an unreachable import fails activation (R-22). Default
    /// `false` means: report the failure, omit this skill, continue.
    #[serde(default)]
    pub required: bool,
}

/// Whether a skill's files live in the namespace tree or come from outside.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillMode {
    /// Skill files live under the namespace's own directory.
    Authored,
    /// Skill files come from a resolved `source` at activation time.
    Imported,
}

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::skills::local::ResolvedSkill;
use crate::skills::source::SourceKind;

/// Resolve an imported skill decl into a `ResolvedSkill`.
///
/// Dispatches by `SourceKind`. Errors propagate; the caller decides whether
/// to apply the `required = true` rule (see `apply_required_rule`).
pub fn resolve_imported_skill<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    decl: &SkillDecl,
) -> Result<ResolvedSkill> {
    if !matches!(decl.mode, SkillMode::Imported) {
        return Err(AenvError::ManifestInvalid(format!(
            "skill '{}' is authored — use authored-skill resolution instead",
            decl.name
        )));
    }
    let source_str = decl.source.as_deref().ok_or_else(|| {
        AenvError::ManifestInvalid(format!("imported skill '{}' has no source", decl.name))
    })?;
    let kind = SourceKind::parse(source_str)?;
    match kind {
        SourceKind::Local(path) => crate::skills::local::resolve_local(fs, &path, &decl.name),
        SourceKind::Git { url, ref_spec } => {
            // Use the decl's ref if provided; else use the URL fragment ref.
            let chosen = decl.ref_.as_deref().or(ref_spec.as_deref());
            crate::skills::git_source::resolve_git(fs, layout, &url, chosen, &decl.name)
        }
        SourceKind::Registry(name) => {
            crate::skills::registry::resolve_registry(&name, decl.ref_.as_deref())
        }
    }
}

/// Resolve, then apply the `required = true` rule.
///
/// Returns:
/// * `Ok(Some(resolution))` when resolution succeeded.
/// * `Ok(None)` when resolution failed AND the skill is not required —
///   caller should emit a warning and continue.
/// * `Err(_)` when resolution failed AND the skill is required.
pub fn apply_required_rule<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    decl: &SkillDecl,
) -> Result<Option<ResolvedSkill>> {
    // Authored decls are always an error — the caller passed the wrong kind.
    if matches!(decl.mode, SkillMode::Authored) {
        return Err(AenvError::ManifestInvalid(format!(
            "skill '{}' is authored — use authored-skill resolution instead",
            decl.name
        )));
    }
    match resolve_imported_skill(fs, layout, decl) {
        Ok(r) => Ok(Some(r)),
        Err(e) => {
            if decl.required {
                Err(e)
            } else {
                Ok(None)
            }
        }
    }
}
