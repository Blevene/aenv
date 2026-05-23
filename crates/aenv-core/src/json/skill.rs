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
    #[serde(skip_serializing_if = "Option::is_none")]
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

use crate::identity::{NamespaceId, QualifiedName, ShortName};
use crate::skills::{SkillDecl, SkillMode};

impl SkillEntry {
    /// Build a `SkillEntry` from a manifest's `[[skills]]` declaration.
    pub fn from_decl(ns: &str, decl: &SkillDecl) -> Self {
        let qn = NamespaceId::new(ns)
            .and_then(|n| ShortName::new(decl.name.clone()).map(|s| QualifiedName::new(n, s)));
        let qualified_name = qn.as_ref().map(ToString::to_string).unwrap_or_default();
        let pin = match (decl.mode, decl.ref_.as_deref()) {
            (_, Some(r)) => Some(r.to_string()),
            (SkillMode::Imported, None) => Some("(head)".to_string()),
            (SkillMode::Authored, None) => None,
        };
        SkillEntry {
            namespace: ns.to_string(),
            qualified_name,
            short_name: decl.name.clone(),
            adapter: decl.adapter.clone(),
            mode: match decl.mode {
                SkillMode::Authored => "authored".into(),
                SkillMode::Imported => "imported".into(),
            },
            source: decl.source.clone(),
            pin,
            required: decl.required,
        }
    }
}
