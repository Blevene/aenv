//! Schema for `aenv get --json`. Matches functional spec §7.1 example.

use crate::parameters::{ParameterValue, ResolvedParameter};
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

impl GetReport {
    /// Build a `GetReport` for one parameter from its `ResolvedParameter`
    /// (the effective value + source namespace) and the inheritance chain
    /// (list of `(namespace_name, value_at_that_namespace)` in chain order).
    pub fn build(
        parameter: String,
        rp: &ResolvedParameter,
        inheritance: Vec<(String, ParameterValue)>,
    ) -> Self {
        GetReport {
            parameter,
            value: param_value_to_json(&rp.value),
            source_namespace: rp.source.as_str().to_string(),
            inheritance_chain: inheritance
                .into_iter()
                .map(|(ns, v)| InheritanceEntry {
                    namespace: ns,
                    value: param_value_to_json(&v),
                })
                .collect(),
        }
    }
}

fn param_value_to_json(v: &ParameterValue) -> serde_json::Value {
    match v {
        ParameterValue::String(s) => serde_json::Value::String(s.clone()),
        ParameterValue::Integer(i) => serde_json::Value::Number((*i).into()),
        ParameterValue::Boolean(b) => serde_json::Value::Bool(*b),
        ParameterValue::ListString(xs) => serde_json::Value::Array(
            xs.iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect(),
        ),
    }
}
