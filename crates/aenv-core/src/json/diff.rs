//! Schema for `aenv diff --json` (project drift) and
//! `aenv diff <a> <b> --json` (structural).

use serde::Serialize;
use std::path::PathBuf;

/// JSON shape for `aenv diff --json` (project-drift flavor).
#[derive(Debug, Clone, Default, Serialize)]
pub struct DriftReport {
    /// Absolute path of the project root.
    pub project: PathBuf,
    /// Name of the currently active namespace.
    pub active_namespace: String,
    /// Files whose on-disk state diverges from the expected materialisation.
    pub drifted: Vec<DriftedFile>,
}

/// One file that has drifted from expected state.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DriftedFile {
    /// Project-relative path.
    pub path: PathBuf,
    /// Qualified name of the artifact (`<namespace>::<short-name>`).
    pub qualified_name: String,
    /// `symlink-replaced`, `merge-regenerated`, `content-divergent`.
    pub kind: String,
    /// Best-effort one-line summary of the divergence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// JSON shape for `aenv diff <a> <b> --json` (structural-diff flavor).
#[derive(Debug, Clone, Default, Serialize)]
pub struct StructuralDiff {
    /// Name of namespace A.
    pub a: String,
    /// Name of namespace B.
    pub b: String,
    /// Skill set differences.
    pub skills: SetDiff,
    /// Agent file differences.
    pub agents: SetDiff,
    /// Parameter value differences.
    pub parameters: ValueDiff,
    /// Policy value differences.
    pub policies: ValueDiff,
    /// Instruction-section set differences.
    pub instructions_sections: SetDiff,
}

/// Set-level diff between two namespaces (added / removed / common).
#[derive(Debug, Clone, Default, Serialize)]
pub struct SetDiff {
    /// Present in B but not in A (`+` in text output).
    pub added: Vec<String>,
    /// Present in A but not in B (`-`).
    pub removed: Vec<String>,
    /// Present in both, may have differing details (`=`).
    pub common: Vec<String>,
}

/// Value-level diff for parameters or policies.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ValueDiff {
    /// Keys present only in B (new).
    pub added: Vec<NamedValue>,
    /// Keys present only in A (deleted).
    pub removed: Vec<NamedValue>,
    /// Keys present in both with different values.
    pub changed: Vec<ValueChange>,
}

/// A named JSON value (used in `ValueDiff::added` / `removed`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct NamedValue {
    /// Parameter or policy key name.
    pub name: String,
    /// The value.
    pub value: serde_json::Value,
}

/// A key whose value changed between namespace A and B.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ValueChange {
    /// Parameter or policy key name.
    pub name: String,
    /// Value in namespace A.
    pub a: serde_json::Value,
    /// Value in namespace B.
    pub b: serde_json::Value,
}
