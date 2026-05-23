//! Resolved-namespace content hash (PRD §5.17, R-84–R-87).
//!
//! Builds the hash input from a `MaterialSet` plus a synthetic
//! `.aenv/parameters.json` entry, prepends the algorithm-version byte,
//! and runs SHA-256. The user-facing form is `sha256-v1:<lowercase-hex>`.

use sha2::{Digest, Sha256};

use crate::jcs::canonicalize;
use crate::materialize::MaterialSet;
use crate::parameters::{ParameterValue, ResolvedParameter};

pub(crate) const ALGORITHM_VERSION_V1: u8 = 0x01;
/// User-facing prefix advertised on every emitted hash string.
pub const HASH_PREFIX_V1: &str = "sha256-v1:";
/// Synthetic path used to fold the resolved parameter map into the hash.
const SYNTHETIC_PARAMETERS_PATH: &str = ".aenv/parameters.json";

/// Compute the resolved-namespace hash per PRD §5.17 R-84.
pub fn hash_resolved_namespace(mat: &MaterialSet) -> String {
    let params_bytes = canonicalize_parameters(&mat.parameters);
    let mut all: Vec<(Vec<u8>, &[u8])> = Vec::with_capacity(mat.entries.len() + 1);
    for (path, content) in &mat.entries {
        all.push((path_to_bytes(path), content.as_slice()));
    }
    all.push((
        SYNTHETIC_PARAMETERS_PATH.as_bytes().to_vec(),
        params_bytes.as_bytes(),
    ));
    all.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    hasher.update([ALGORITHM_VERSION_V1]);
    for (path_bytes, content) in &all {
        let path_len: u32 = u32::try_from(path_bytes.len())
            .expect("path length exceeds u32::MAX — impossible on real filesystems");
        let content_len: u64 =
            u64::try_from(content.len()).expect("content length exceeds u64::MAX");
        hasher.update(path_len.to_be_bytes());
        hasher.update(path_bytes);
        hasher.update(content_len.to_be_bytes());
        hasher.update(content);
    }
    let digest = hasher.finalize();
    format!("{HASH_PREFIX_V1}{}", HexDisplay(&digest))
}

fn canonicalize_parameters(
    params: &std::collections::BTreeMap<String, ResolvedParameter>,
) -> String {
    let mut map = serde_json::Map::with_capacity(params.len());
    for (k, rp) in params {
        // Intentionally omit rp.source — the hash captures effective values
        // only, not which namespace in the override chain supplied them.
        // See engineering §7.5 (hash-neutrality invariant).
        map.insert(k.clone(), parameter_value_to_json(&rp.value));
    }
    canonicalize(&serde_json::Value::Object(map))
}

fn parameter_value_to_json(v: &ParameterValue) -> serde_json::Value {
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

fn path_to_bytes(p: &std::path::Path) -> Vec<u8> {
    // Normalize Windows path separators to forward slashes so a hash
    // computed on Windows matches one computed on Unix. Backslash is a
    // legal filename character on Unix but is unsupported in aenv
    // namespace directories — using it would produce a hash collision
    // with the equivalent forward-slash path.
    let s = p.to_string_lossy();
    s.replace('\\', "/").into_bytes()
}

struct HexDisplay<'a>(&'a [u8]);

impl std::fmt::Display for HexDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}
