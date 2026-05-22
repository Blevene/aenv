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
use crate::identity::NamespaceId;
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
pub fn policy_table_from_toml(
    raw: &BTreeMap<String, toml::Value>,
) -> Result<BTreeMap<String, PolicyDecl>> {
    let mut out = BTreeMap::new();
    for (k, v) in raw {
        out.insert(k.clone(), PolicyDecl::from_toml_value(k, v)?);
    }
    Ok(out)
}

/// One resolved policy after `extends`-chain resolution.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolvedPolicy {
    /// Effective value after `extends`-chain resolution.
    pub value: PolicyValue,
    /// Final `enforce` flag (`true` if any namespace in the chain set it).
    pub enforce: bool,
    /// Latest namespace in the chain that declared this key.
    pub source: NamespaceId,
}

impl ResolvedPolicy {
    /// Human-readable rendering of the effective policy value.
    pub fn value_display(&self) -> String {
        match &self.value {
            PolicyValue::Integer(i) => i.to_string(),
            PolicyValue::Boolean(b) => b.to_string(),
            PolicyValue::ListString(xs) => format!(
                "[{}]",
                xs.iter()
                    .map(|s| format!("\"{s}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// Resolve `[policies]` tables across the `extends` chain. Returns
/// `ManifestInvalid` if any child weakens a parent's `enforce = true`
/// declaration (R-75) or if the same key has incompatible types across the chain.
///
/// "Weaken" is defined per type:
/// * Integer: raising the limit (`child > parent`) weakens.
/// * Boolean: flipping `true` -> `false` weakens.
/// * ListString: removing entries weakens (the child must be a superset).
/// * Enforce flag: changing `true` -> `false` weakens, even with the same value.
pub fn resolve_policies(
    chain: &[NamespaceId],
    per_ns: &BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>>,
) -> Result<BTreeMap<String, ResolvedPolicy>> {
    let mut out: BTreeMap<String, ResolvedPolicy> = BTreeMap::new();
    for ns in chain {
        let table = match per_ns.get(ns) {
            Some(t) => t,
            None => continue,
        };
        for (k, decl) in table {
            if let Some(prev) = out.get(k) {
                if prev.value.type_tag() != decl.value.type_tag() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "policy '{}' has incompatible types across chain: \
                         {} declared {} but {} declared {}",
                        k,
                        prev.source,
                        prev.value.type_tag(),
                        ns,
                        decl.value.type_tag()
                    )));
                }
                if prev.enforce {
                    enforce_protection(k, ns, prev, decl)?;
                }
            }
            out.insert(
                k.clone(),
                ResolvedPolicy {
                    value: decl.value.clone(),
                    enforce: decl.enforce,
                    source: ns.clone(),
                },
            );
        }
    }
    Ok(out)
}

pub mod builtin;

fn enforce_protection(
    key: &str,
    child_ns: &NamespaceId,
    parent: &ResolvedPolicy,
    child: &PolicyDecl,
) -> Result<()> {
    // The parent is enforced. The child may keep enforce on or raise it; it
    // may not downgrade to advisory.
    if !child.enforce {
        return Err(AenvError::ManifestInvalid(format!(
            "policy '{}' is enforced by {} but {} sets enforce = false (R-75: \
             a child may not downgrade an inherited enforced policy)",
            key, parent.source, child_ns
        )));
    }

    // Same-or-stricter check by type.
    match (&parent.value, &child.value) {
        (PolicyValue::Integer(p), PolicyValue::Integer(c)) => {
            if c > p {
                return Err(AenvError::ManifestInvalid(format!(
                    "policy '{key}' is enforced by {} at {p}; {} attempts to weaken \
                     by raising the limit to {c} (R-75)",
                    parent.source, child_ns
                )));
            }
        }
        (PolicyValue::Boolean(p), PolicyValue::Boolean(c)) => {
            if *p && !*c {
                return Err(AenvError::ManifestInvalid(format!(
                    "policy '{key}' is enforced by {} at true; {} attempts to weaken \
                     to false (R-75)",
                    parent.source, child_ns
                )));
            }
        }
        (PolicyValue::ListString(p_list), PolicyValue::ListString(c_list)) => {
            for parent_entry in p_list {
                if !c_list.contains(parent_entry) {
                    return Err(AenvError::ManifestInvalid(format!(
                        "policy '{key}' is enforced by {} and includes '{parent_entry}'; \
                         {} attempts to weaken by removing it (R-75)",
                        parent.source, child_ns
                    )));
                }
            }
        }
        // Type-mismatch already caught by the caller.
        _ => unreachable!("type-mismatch should have been caught earlier"),
    }
    Ok(())
}
