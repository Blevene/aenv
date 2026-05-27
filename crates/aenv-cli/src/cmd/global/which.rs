//! `aenv global which <path>` — show which namespace manages a user-scope path.

use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::{Path, PathBuf};

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
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
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "scope": "user",
                        "path": format!("~/{}", m.path.display()),
                        "qualified": m.qualified_name.to_string(),
                        "strategy": m.strategy,
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
