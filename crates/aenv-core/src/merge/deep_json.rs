//! Deep-merge for JSON.
//!
//! Two-space pretty print on output so editor diffs read cleanly and so that
//! Phase 5's hashing canonicalization has a stable starting point.

use serde_json::Value;

use super::MergeError;

/// Merge a sequence of JSON byte arrays using deep-merge rules.
///
/// Rules:
/// - Two objects: union of keys, recursive merge on overlap.
/// - Two arrays: concatenate in chain order.
/// - Type mismatch: later value wins.
/// - `null` + anything: anything wins.
///
/// Returns two-space pretty-printed JSON with trailing newline.
pub fn merge_json(inputs: &[Vec<u8>]) -> Result<Vec<u8>, MergeError> {
    if inputs.is_empty() {
        return Ok(b"{}\n".to_vec());
    }
    let mut acc: Option<Value> = None;
    for bytes in inputs {
        let v: Value = serde_json::from_slice(bytes).map_err(|e| MergeError::Parse {
            kind: "json",
            detail: e.to_string(),
        })?;
        acc = Some(match acc.take() {
            None => v,
            Some(existing) => deep_merge_value(existing, v),
        });
    }
    let merged = acc.unwrap_or(Value::Object(Default::default()));
    let mut out = serde_json::to_vec_pretty(&merged).map_err(|e| MergeError::Parse {
        kind: "json",
        detail: e.to_string(),
    })?;
    out.push(b'\n');
    Ok(out)
}

pub(crate) fn deep_merge_value(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Object(mut am), Value::Object(bm)) => {
            for (k, bv) in bm {
                let merged = match am.remove(&k) {
                    Some(av) => deep_merge_value(av, bv),
                    None => bv,
                };
                am.insert(k, merged);
            }
            Value::Object(am)
        }
        (Value::Array(mut aa), Value::Array(ba)) => {
            aa.extend(ba);
            Value::Array(aa)
        }
        (Value::Null, b) => b,
        (_, b) => b, // last-wins on type mismatch
    }
}
