//! `aenv global which <path>` — show which namespace manages a user-scope path.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::{Path, PathBuf};

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    _fake_home: &Path,
    path: &Path,
    json: bool,
) -> Result<()> {
    let state_path = layout.global_state_path();
    if !fs.exists(&state_path)? {
        if json {
            println!("{}", serde_json::json!({"scope": "user", "active": false}));
        } else {
            println!("no global activation");
        }
        return Ok(());
    }
    let bytes = fs.read(&state_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("global-state.json: {e}")))?;
    let state = aenv_core::state::ActivationState::from_json(text)?;

    let normalized = normalize_query(path);
    let hit = state.managed_files.iter().find(|m| m.path == normalized);
    match hit {
        Some(m) => {
            if json {
                // Compute the resolved bytes for this single path. We use the
                // namespace-level material set (option (a) in Task 19's plan)
                // — simpler than factoring out a per-path helper, with
                // negligible perf cost on the user-visible `aenv global which`
                // path. The hash matches what `aenv global activate` would
                // have written to disk.
                let active_ns =
                    aenv_core::identity::NamespaceId::new(state.active_namespace.as_str())
                        .map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
                let mat = aenv_core::materialize::compute_material_set_user(
                    fs, layout, adapters, &active_ns,
                )?;
                let content_hash = mat
                    .entries()
                    .iter()
                    .find(|(p, _)| p == &m.path)
                    .map(|(_, bytes)| sha256_hex(bytes));

                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "scope": "user",
                        "path": format!("~/{}", m.path.display()),
                        "qualified": m.qualified_name.to_string(),
                        "strategy": m.strategy,
                        "content_hash": content_hash,
                    }))
                    .unwrap()
                );
            } else {
                println!("~/{} -> {}", m.path.display(), m.qualified_name);
            }
            Ok(())
        }
        None => {
            if json {
                println!("{}", serde_json::json!({"scope": "user", "managed": false}));
            } else {
                println!("not managed by the active global namespace");
            }
            Ok(())
        }
    }
}

fn normalize_query(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        PathBuf::from(rest)
    } else if let Some(rest) = s.strip_prefix('/') {
        // Absolute path probably means $HOME-rooted; strip nothing else — let
        // the lookup miss naturally if it doesn't match a managed file.
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// SHA-256 of `bytes`, hex-encoded, with a `sha256:` prefix. Distinct framing
/// from `aenv_core::hash::hash_resolved_namespace` (which sorts + length-
/// prefixes path entries under an algorithm-version byte) — for a single
/// file's bytes, plain SHA-256 is correct and unambiguous.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity("sha256:".len() + digest.len() * 2);
    s.push_str("sha256:");
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
