//! Schema for `aenv list --json`. Matches functional spec §7.1.

use serde::Serialize;

/// JSON shape for one namespace row in `aenv list --json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ListEntry {
    /// Fully-qualified namespace name.
    pub name: String,
    /// Namespaces this namespace extends, in declaration order.
    pub extends: Vec<String>,
    /// Adapters declared in this namespace.
    pub adapters: Vec<String>,
    /// Parameter keys declared directly (not inherited).
    pub parameters_declared: Vec<String>,
    /// Policy keys declared directly (not inherited).
    pub policies_declared: Vec<String>,
    /// `sha256-v1:<hex>` of the resolved namespace. Absent if resolution failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash: Option<String>,
    /// R-87 forward-compatibility hook (always None in v1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash_v2: Option<String>,
    /// If resolution failed, the error message lands here. The entry is
    /// still emitted so a script gets every namespace, not just the
    /// healthy ones.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
