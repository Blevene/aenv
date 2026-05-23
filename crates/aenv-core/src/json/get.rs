//! Schema for `aenv get --json`. Matches functional spec §7.1 example.

use serde::Serialize;

/// JSON shape for `aenv get <param> --json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GetReport {
    /// Parameter key that was looked up.
    pub parameter: String,
    /// JSON value as declared (string/int/bool/array).
    pub value: serde_json::Value,
    /// Namespace in the `extends` chain that supplied the effective value.
    pub source_namespace: String,
    /// All contributions from root to leaf, in resolution order.
    pub inheritance_chain: Vec<InheritanceEntry>,
}

/// One step in the inheritance chain for a parameter.
#[derive(Debug, Clone, Default, Serialize)]
pub struct InheritanceEntry {
    /// Namespace that declared the value at this step.
    pub namespace: String,
    /// Value declared by this namespace.
    pub value: serde_json::Value,
}
