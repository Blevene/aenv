//! Policy declarations on a namespace manifest.
//!
//! `[policies]` accepts two shapes per key:
//!
//! ```toml
//! [policies]
//! instructions_max_chars = 3000                                    # advisory
//! skill_requires_description = { value = true, enforce = true }    # enforced
//! ```
//!
//! Phase 3 understands four built-in policy keys (`instructions_max_chars`,
//! `skill_requires_description`, `mcp_requires_command_or_url`, `forbid_paths`).
//! Unknown keys parse but are skipped by the evaluator (`aenv doctor` emits
//! a warning).

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Value side of a policy declaration. Integer, boolean, or list-of-string.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolicyValue {
    /// Integer policy value (e.g. `instructions_max_chars = 3000`).
    Integer(i64),
    /// Boolean policy value (e.g. `skill_requires_description = true`).
    Boolean(bool),
    /// List-of-string policy value (e.g. `forbid_paths = ["secrets/**"]`).
    ListString(Vec<String>),
}

impl PolicyValue {
    /// One of "integer", "boolean", "list-of-string".
    pub fn type_tag(&self) -> &'static str {
        match self {
            PolicyValue::Integer(_) => "integer",
            PolicyValue::Boolean(_) => "boolean",
            PolicyValue::ListString(_) => "list-of-string",
        }
    }
}

/// One policy declaration in a manifest's `[policies]` table.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolicyDecl {
    /// The policy's value (type depends on the key).
    pub value: PolicyValue,
    /// `enforce = true` makes activation refuse on violation.
    #[serde(default)]
    pub enforce: bool,
}

impl PolicyDecl {
    /// Convert a `toml::Value` (which may be a value or a `{ value, enforce }`
    /// table) into a `PolicyDecl`. Returns `ManifestInvalid` for unsupported
    /// shapes.
    pub fn from_toml_value(key: &str, v: &toml::Value) -> Result<Self> {
        if let toml::Value::Table(t) = v {
            // Table form: must have `value`; `enforce` defaults to false.
            let value_tv = t.get("value").ok_or_else(|| {
                AenvError::ManifestInvalid(format!(
                    "policy '{key}' table-form is missing 'value' field"
                ))
            })?;
            let value = policy_value_from_toml(key, value_tv)?;
            let enforce = match t.get("enforce") {
                Some(toml::Value::Boolean(b)) => *b,
                Some(other) => {
                    return Err(AenvError::ManifestInvalid(format!(
                        "policy '{key}' has non-boolean 'enforce' field ({})",
                        toml_type_name(other)
                    )));
                }
                None => false,
            };
            // Reject any other unexpected fields in the table to surface typos.
            for k in t.keys() {
                if k != "value" && k != "enforce" {
                    return Err(AenvError::ManifestInvalid(format!(
                        "policy '{key}' has unknown field '{k}' (only 'value' and 'enforce' are accepted)"
                    )));
                }
            }
            Ok(PolicyDecl { value, enforce })
        } else {
            // Shorthand: bare value, advisory.
            Ok(PolicyDecl {
                value: policy_value_from_toml(key, v)?,
                enforce: false,
            })
        }
    }
}

fn policy_value_from_toml(key: &str, v: &toml::Value) -> Result<PolicyValue> {
    match v {
        toml::Value::Integer(i) => Ok(PolicyValue::Integer(*i)),
        toml::Value::Boolean(b) => Ok(PolicyValue::Boolean(*b)),
        toml::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for (i, elem) in arr.iter().enumerate() {
                match elem {
                    toml::Value::String(s) => out.push(s.clone()),
                    other => {
                        return Err(AenvError::ManifestInvalid(format!(
                            "policy '{key}' list element {i} is {} but only list-of-string is supported",
                            toml_type_name(other)
                        )));
                    }
                }
            }
            Ok(PolicyValue::ListString(out))
        }
        other => Err(AenvError::ManifestInvalid(format!(
            "policy '{key}' has unsupported value type {}; \
             only integer, boolean, list-of-string (or {{ value = ..., enforce = bool }}) are supported",
            toml_type_name(other)
        ))),
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

/// Convenience: parse every entry in a `BTreeMap<String, toml::Value>`
/// (typically from a manifest's `[policies]` table) into typed `PolicyDecl`s.
pub fn parse_policy_table(
    raw: &BTreeMap<String, toml::Value>,
) -> Result<BTreeMap<String, PolicyDecl>> {
    let mut out = BTreeMap::new();
    for (k, v) in raw {
        out.insert(k.clone(), PolicyDecl::from_toml_value(k, v)?);
    }
    Ok(out)
}
