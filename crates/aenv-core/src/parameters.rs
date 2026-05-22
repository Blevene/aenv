//! Typed parameters declared in a namespace's `[parameters]` table.
//!
//! Phase 3 supports four TOML types — string, integer, boolean, list-of-string
//! — and rejects everything else (`float`, `datetime`, `table`, mixed-type
//! arrays) at parse time. Adapters declare which parameters they consume via
//! `Adapter::parameters`; the resolver then enforces type-compat (R-71).

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A typed parameter value, parsed from a `[parameters]` table entry.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue {
    /// String value, e.g. `default_model = "claude-opus-4.7"`.
    String(String),
    /// Integer value, e.g. `instructions_budget = 3000`.
    Integer(i64),
    /// Boolean value, e.g. `auto_invoke_subagents = true`.
    Boolean(bool),
    /// Homogeneous list of strings, e.g. `forbid_tools = ["edit", "write"]`.
    ListString(Vec<String>),
}

impl ParameterValue {
    /// Convert a `toml::Value` into a `ParameterValue`, rejecting unsupported
    /// shapes. Returns `ManifestInvalid` with a human-readable reason.
    pub fn from_toml_value(v: &toml::Value) -> Result<Self> {
        match v {
            toml::Value::String(s) => Ok(ParameterValue::String(s.clone())),
            toml::Value::Integer(i) => Ok(ParameterValue::Integer(*i)),
            toml::Value::Boolean(b) => Ok(ParameterValue::Boolean(*b)),
            toml::Value::Array(arr) => {
                let mut out = Vec::with_capacity(arr.len());
                for (i, elem) in arr.iter().enumerate() {
                    match elem {
                        toml::Value::String(s) => out.push(s.clone()),
                        other => {
                            return Err(AenvError::ManifestInvalid(format!(
                                "parameter list element {i} is {} but only list-of-string is supported",
                                toml_type_name(other)
                            )));
                        }
                    }
                }
                Ok(ParameterValue::ListString(out))
            }
            toml::Value::Float(_) => Err(AenvError::ManifestInvalid(
                "parameter has float type; only string, integer, boolean, list-of-string are supported"
                    .into(),
            )),
            toml::Value::Datetime(_) => Err(AenvError::ManifestInvalid(
                "parameter has datetime type; only string, integer, boolean, list-of-string are supported"
                    .into(),
            )),
            toml::Value::Table(_) => Err(AenvError::ManifestInvalid(
                "parameter has table type; only string, integer, boolean, list-of-string are supported"
                    .into(),
            )),
        }
    }

    /// One of "string", "integer", "boolean", "list-of-string". Used in
    /// error messages and for type-compat checks against adapter declarations.
    pub fn type_tag(&self) -> &'static str {
        match self {
            ParameterValue::String(_) => "string",
            ParameterValue::Integer(_) => "integer",
            ParameterValue::Boolean(_) => "boolean",
            ParameterValue::ListString(_) => "list-of-string",
        }
    }
}

impl fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterValue::String(s) => write!(f, "{s}"),
            ParameterValue::Integer(i) => write!(f, "{i}"),
            ParameterValue::Boolean(b) => write!(f, "{b}"),
            ParameterValue::ListString(xs) => {
                let parts: Vec<String> = xs.iter().map(|s| format!("\"{s}\"")).collect();
                write!(f, "[{}]", parts.join(", "))
            }
        }
    }
}

fn toml_type_name(v: &toml::Value) -> &'static str {
    match v {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}

use crate::adapter::AdapterRegistry;
use crate::identity::NamespaceId;
use std::collections::BTreeMap;

/// One resolved parameter: value + the namespace in the `extends` chain that
/// supplied it.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolvedParameter {
    /// Effective value after `extends`-chain resolution.
    pub value: ParameterValue,
    /// Latest namespace in the chain that declared this key.
    pub source: NamespaceId,
}

/// Collapse per-namespace `[parameters]` tables into a single map of effective
/// values. `chain` is in root → leaf order (the order `resolve_namespace`
/// produces). `per_ns` must contain an entry for every namespace in `chain`,
/// even if that entry is empty.
///
/// Semantics:
/// * Last-wins per-key (PRD R-67). The leaf overrides the root.
/// * Type-incompat across the chain (parent declares `int`, child declares
///   `string`) is a `ManifestInvalid` error. Same-type overrides are fine.
///
/// This function does NOT consult adapter declarations; that's Task 5
/// (full R-71 enforcement).
pub fn resolve_parameters(
    chain: &[NamespaceId],
    per_ns: &BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>>,
) -> Result<BTreeMap<String, ResolvedParameter>> {
    let mut out: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    for ns in chain {
        let table = match per_ns.get(ns) {
            Some(t) => t,
            None => continue,
        };
        for (k, v) in table {
            if let Some(prev) = out.get(k) {
                if prev.value.type_tag() != v.type_tag() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "parameter '{}' has incompatible types across chain: \
                         {} declared {} but {} declared {}",
                        k,
                        prev.source,
                        prev.value.type_tag(),
                        ns,
                        v.type_tag()
                    )));
                }
            }
            out.insert(
                k.clone(),
                ResolvedParameter {
                    value: v.clone(),
                    source: ns.clone(),
                },
            );
        }
    }
    Ok(out)
}

/// Verify that every adapter-declared parameter has a manifest value of the
/// adapter-declared type (PRD R-71). Manifest-only parameters (not declared
/// by any adapter) are allowed.
///
/// Also rejects the case where two adapters declare the same parameter name
/// with different types — that's a configuration bug.
pub fn check_against_adapters(
    resolved: &BTreeMap<String, ResolvedParameter>,
    adapters: &AdapterRegistry,
) -> Result<()> {
    // Build a map: parameter name -> (adapter_name, type_tag).
    let mut decls: BTreeMap<&str, (&str, &'static str)> = BTreeMap::new();
    for (adapter_name, adapter) in adapters.iter() {
        for p in &adapter.parameters {
            if let Some((other_adapter, other_type)) = decls.get(p.name.as_str()) {
                if *other_type != p.r#type.type_tag() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "parameter '{}' is declared by adapters '{}' ({}) \
                         and '{}' ({}) with conflicting types",
                        p.name,
                        other_adapter,
                        other_type,
                        adapter_name,
                        p.r#type.type_tag()
                    )));
                }
            } else {
                decls.insert(p.name.as_str(), (adapter_name.as_str(), p.r#type.type_tag()));
            }
        }
    }

    for (name, rp) in resolved {
        if let Some((adapter_name, decl_type)) = decls.get(name.as_str()) {
            if *decl_type != rp.value.type_tag() {
                return Err(AenvError::ManifestInvalid(format!(
                    "parameter '{}' has type {} in namespace {} but adapter '{}' \
                     declared it as {}",
                    name,
                    rp.value.type_tag(),
                    rp.source,
                    adapter_name,
                    decl_type
                )));
            }
        }
    }

    Ok(())
}
