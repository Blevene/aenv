//! Deep-merge for TOML.
//!
//! `toml::Value` and `serde_json::Value` share a structural model. We convert
//! TOML -> JSON, reuse `deep_merge_value`, then convert back.

use serde_json::Value as JsonValue;

use super::{deep_json::deep_merge_value, MergeError};

/// Merge a sequence of TOML byte arrays using deep-merge rules.
///
/// Rules:
/// - Two objects: union of keys, recursive merge on overlap.
/// - Two arrays: concatenate in chain order.
/// - Type mismatch: later value wins.
/// - `null` + anything: anything wins.
///
/// Returns TOML output via `toml::to_string_pretty`.
pub fn merge_toml(inputs: &[Vec<u8>]) -> Result<Vec<u8>, MergeError> {
    if inputs.is_empty() {
        return Ok(b"".to_vec());
    }
    let mut acc: Option<JsonValue> = None;
    for bytes in inputs {
        let text = std::str::from_utf8(bytes).map_err(|e| MergeError::Utf8(e.to_string()))?;
        let tv: toml::Value = toml::from_str(text).map_err(|e| MergeError::Parse {
            kind: "toml",
            detail: e.to_string(),
        })?;
        let jv: JsonValue = serde_json::to_value(&tv).map_err(|e| MergeError::Parse {
            kind: "toml",
            detail: format!("toml -> json failed: {e}"),
        })?;
        acc = Some(match acc.take() {
            None => jv,
            Some(existing) => deep_merge_value(existing, jv),
        });
    }
    let merged = acc.unwrap_or(JsonValue::Object(Default::default()));
    let merged_toml: toml::Value =
        serde_json::from_value(merged).map_err(|e| MergeError::Parse {
            kind: "toml",
            detail: format!("json -> toml failed: {e}"),
        })?;
    let out = toml::to_string_pretty(&merged_toml).map_err(|e| MergeError::Parse {
        kind: "toml",
        detail: e.to_string(),
    })?;
    Ok(out.into_bytes())
}
