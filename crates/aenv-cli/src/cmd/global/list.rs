//! `aenv global list` — namespaces that declare user-scope files.

use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, json: bool) -> Result<()> {
    let envs_dir = layout.namespaces_dir();
    let mut names: Vec<String> = Vec::new();
    if fs.exists(&envs_dir)? {
        for path in fs.list_dir(&envs_dir)? {
            let manifest_path = path.join("aenv.toml");
            if !fs.exists(&manifest_path).unwrap_or(false) {
                continue;
            }
            let bytes = match fs.read(&manifest_path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let text = match std::str::from_utf8(&bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let manifest = match aenv_core::manifest::AenvManifest::from_toml(text) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let has_user = manifest.adapters.values().any(|e| !e.user_files.is_empty());
            if has_user {
                names.push(manifest.name);
            }
        }
    }
    names.sort();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "scope": "user",
                "namespaces": names,
            }))
            .unwrap()
        );
    } else if names.is_empty() {
        println!("(no namespaces declare user_files)");
    } else {
        for n in &names {
            println!("{n}");
        }
    }
    Ok(())
}
