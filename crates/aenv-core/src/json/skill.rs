//! Schema for `aenv skill list --json`.

use serde::Serialize;

/// JSON shape for one skill row in `aenv skill list --json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SkillEntry {
    /// Namespace that owns or imports the skill.
    pub namespace: String,
    /// Qualified name `<namespace>::<short-name>`.
    pub qualified_name: String,
    /// Short name of the skill (without namespace prefix).
    pub short_name: String,
    /// Adapter this skill belongs to, if any.
    pub adapter: Option<String>,
    /// `authored` or `imported`.
    pub mode: String,
    /// Source identifier for imported skills (git URL, local path, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Pinned ref string (only for imported). `(head)` for unpinned imports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// Whether the skill is declared `required = true`.
    pub required: bool,
}
