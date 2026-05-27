//! `aenv global status` — current user-scope activation.

use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    fake_home: &Path,
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
    if json {
        let payload = serde_json::json!({
            "scope": "user",
            "active": true,
            "active_namespace": state.active_namespace,
            "target_root": fake_home,
            "managed_files": state.managed_files.iter().map(|m| serde_json::json!({
                "path": m.path,
                "strategy": m.strategy,
            })).collect::<Vec<_>>(),
            "backed_up": state.backed_up.iter().map(|b| serde_json::json!({
                "original_path": b.original_path,
                "backup_path": b.backup_path,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else {
        println!("Active global namespace: {}", state.active_namespace);
        println!("Target root: {}", fake_home.display());
        println!("Managed files: {}", state.managed_files.len());
        for m in &state.managed_files {
            println!("  ~/{}", m.path.display());
        }
        println!("Note: running harness sessions retain their previous config until restart.");
    }
    Ok(())
}
