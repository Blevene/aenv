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
