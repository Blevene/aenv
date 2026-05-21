//! Deep-merge for YAML: parse YAML -> serde_json::Value -> deep_merge_value
//! -> emit YAML.
//!
//! YAML tagged scalars (timestamps, binary, custom tags) round-trip lossily
//! through serde_json::Value; this is acceptable for Phase 2's targets
//! (.aider.conf.yml and friends) which use only plain scalars + maps +
//! sequences.

use serde_json::Value as JsonValue;

use super::{deep_json::deep_merge_value, MergeError};

/// Merge a sequence of YAML byte arrays using deep-merge rules.
///
/// Rules:
/// - Two objects: union of keys, recursive merge on overlap.
/// - Two arrays: concatenate in chain order.
/// - Type mismatch: later value wins.
/// - `null` + anything: anything wins.
///
/// Returns YAML output with trailing newline.
pub fn merge_yaml(inputs: &[Vec<u8>]) -> Result<Vec<u8>, MergeError> {
    if inputs.is_empty() {
        return Ok(b"{}\n".to_vec());
    }
    let mut acc: Option<JsonValue> = None;
    for bytes in inputs {
        let yv: serde_yaml::Value =
            serde_yaml::from_slice(bytes).map_err(|e| MergeError::Parse {
                kind: "yaml",
                detail: e.to_string(),
            })?;
        let jv: JsonValue = serde_json::to_value(&yv).map_err(|e| MergeError::Parse {
            kind: "yaml",
            detail: format!("yaml -> json conversion failed: {e}"),
        })?;
        acc = Some(match acc.take() {
            None => jv,
            Some(existing) => deep_merge_value(existing, jv),
        });
    }
    let merged = acc.unwrap_or(JsonValue::Object(Default::default()));
    let merged_yaml: serde_yaml::Value =
        serde_json::from_value(merged).map_err(|e| MergeError::Parse {
            kind: "yaml",
            detail: format!("json -> yaml conversion failed: {e}"),
        })?;
    let out = serde_yaml::to_string(&merged_yaml).map_err(|e| MergeError::Parse {
        kind: "yaml",
        detail: e.to_string(),
    })?;
    Ok(out.into_bytes())
}
