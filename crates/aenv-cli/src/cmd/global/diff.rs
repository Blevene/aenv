//! `aenv global diff` — user-scope drift (no args) or structural diff (two
//! namespace names).
//!
//! Initial user-scope diff: path-set diff only. Byte-perfect diff parity with
//! `aenv diff` is a follow-up.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::resolve_namespace;
use aenv_core::scope::Scope;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    fake_home: &Path,
    ns_a: Option<&str>,
    ns_b: Option<&str>,
    json: bool,
) -> Result<()> {
    match (ns_a, ns_b) {
        (None, None) => run_drift(fs, layout, fake_home, json),
        (Some(a), Some(b)) => run_structural(fs, layout, a, b, json),
        _ => Err(AenvError::ManifestInvalid(
            "aenv global diff needs either zero or two namespace arguments".into(),
        )),
    }
}

fn run_drift<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    fake_home: &Path,
    json: bool,
) -> Result<()> {
    let state_path = layout.global_state_path();
    if !fs.exists(&state_path)? {
        if json {
            println!(
                "{}",
                serde_json::json!({"scope": "user", "active": false, "drifted": []})
            );
        } else {
            println!("no global activation");
        }
        return Ok(());
    }
    let bytes = fs.read(&state_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("global-state.json: {e}")))?;
    let state = aenv_core::state::ActivationState::from_json(text)?;

    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let leaf = NamespaceId::new(&state.active_namespace)
        .map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    let resolution = resolve_namespace(fs, layout, &adapters, &leaf)?;

    // Index user-scope source bytes by relative path. Last-wins along the
    // resolution chain (matches activation semantics).
    let mut source_by_path: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    for c in resolution
        .candidates
        .iter()
        .filter(|c| c.scope == Scope::User)
    {
        if let Ok(b) = fs.read(&c.source_path) {
            source_by_path.insert(c.path.clone(), b);
        }
    }

    let mut drifted: Vec<DriftEntry> = Vec::new();
    for m in &state.managed_files {
        let on_disk_path = fake_home.join(&m.path);
        let on_disk = fs.read(&on_disk_path).ok();
        let source = source_by_path.get(&m.path).cloned();
        match (on_disk, source) {
            (None, _) => drifted.push(DriftEntry {
                path: m.path.clone(),
                kind: "missing".into(),
            }),
            (Some(d), Some(s)) if d != s => drifted.push(DriftEntry {
                path: m.path.clone(),
                kind: "modified".into(),
            }),
            (Some(_), None) => drifted.push(DriftEntry {
                path: m.path.clone(),
                kind: "no-source".into(),
            }),
            (Some(_), Some(_)) => {}
        }
    }

    if json {
        let payload = serde_json::json!({
            "scope": "user",
            "active": true,
            "active_namespace": state.active_namespace,
            "drifted": drifted.iter().map(|d| serde_json::json!({
                "path": d.path,
                "kind": d.kind,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else if drifted.is_empty() {
        println!("No drift detected. Active global namespace matches its source.");
    } else {
        println!("Drift in global activation '{}':", state.active_namespace);
        for d in &drifted {
            println!("  ~/{} ({})", d.path.display(), d.kind);
        }
    }
    Ok(())
}

fn run_structural<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    a: &str,
    b: &str,
    json: bool,
) -> Result<()> {
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let leaf_a = NamespaceId::new(a).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    let leaf_b = NamespaceId::new(b).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    let res_a = resolve_namespace(fs, layout, &adapters, &leaf_a)?;
    let res_b = resolve_namespace(fs, layout, &adapters, &leaf_b)?;

    let mut paths_a: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    for c in res_a.candidates.iter().filter(|c| c.scope == Scope::User) {
        if let Ok(bytes) = fs.read(&c.source_path) {
            paths_a.insert(c.path.clone(), bytes);
        }
    }
    let mut paths_b: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    for c in res_b.candidates.iter().filter(|c| c.scope == Scope::User) {
        if let Ok(bytes) = fs.read(&c.source_path) {
            paths_b.insert(c.path.clone(), bytes);
        }
    }
    let keys_a: BTreeSet<&PathBuf> = paths_a.keys().collect();
    let keys_b: BTreeSet<&PathBuf> = paths_b.keys().collect();
    let added: Vec<PathBuf> = keys_b.difference(&keys_a).map(|p| (*p).clone()).collect();
    let removed: Vec<PathBuf> = keys_a.difference(&keys_b).map(|p| (*p).clone()).collect();
    let mut changed: Vec<PathBuf> = Vec::new();
    for p in keys_a.intersection(&keys_b) {
        if paths_a.get(*p) != paths_b.get(*p) {
            changed.push((*p).clone());
        }
    }

    if json {
        let payload = serde_json::json!({
            "scope": "user",
            "a": a,
            "b": b,
            "added": added,
            "removed": removed,
            "changed": changed,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else {
        if added.is_empty() && removed.is_empty() && changed.is_empty() {
            println!("No user-scope differences between '{a}' and '{b}'.");
        } else {
            println!("User-scope diff '{a}' vs '{b}':");
            for p in &added {
                println!("  +{}", p.display());
            }
            for p in &removed {
                println!("  -{}", p.display());
            }
            for p in &changed {
                println!("  ~{}", p.display());
            }
        }
    }
    Ok(())
}

struct DriftEntry {
    path: PathBuf,
    kind: String,
}
