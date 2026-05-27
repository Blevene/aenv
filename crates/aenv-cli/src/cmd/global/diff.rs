//! `aenv global diff` — user-scope drift and structural diff.
//!
//! Drift mode (no args): compares each managed file's bytes on disk under
//! `$HOME/` against the resolved source bytes from the active namespace.
//! Structural mode (`ns_a ns_b` args): path-set diff between two namespaces'
//! user-scope subsets — does not yet inspect bytes. Per-byte structural
//! diff parity with `aenv diff` is a follow-up.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::compute_material_set_user;
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
                serde_json::json!({"scope": "user", "active": false, "status": "inactive", "files": []})
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

    // Use the same post-merge material set the activate flow produces.
    // This gives correct byte-level comparison for SectionMerge / DeepMerge
    // strategies, not just Symlink/Identical.
    let mat = compute_material_set_user(fs, layout, &adapters, &leaf)?;
    let source_by_path: BTreeMap<PathBuf, &[u8]> = mat
        .entries()
        .iter()
        .map(|(p, c)| (p.clone(), c.as_slice()))
        .collect();

    // Categorize each managed file against the resolved bytes.
    let mut files: Vec<DriftFile> = Vec::new();
    for m in &state.managed_files {
        let on_disk_path = fake_home.join(&m.path);
        let on_disk = fs.read(&on_disk_path).ok();
        let source = source_by_path.get(&m.path).copied();
        let state_kind = match (on_disk, source) {
            (None, _) => "missing",
            (Some(_), None) => "unexpected",
            (Some(d), Some(s)) if d.as_slice() != s => "modified",
            (Some(_), Some(_)) => "unchanged",
        };
        files.push(DriftFile {
            path: m.path.clone(),
            state: state_kind.to_string(),
        });
    }

    let drifted_count = files.iter().filter(|f| f.state != "unchanged").count();
    let status = if drifted_count == 0 { "clean" } else { "drift" };

    if json {
        let payload = serde_json::json!({
            "scope": "user",
            "active": true,
            "active_namespace": state.active_namespace,
            "status": status,
            "files": files.iter().map(|f| serde_json::json!({
                "path": f.path,
                "state": f.state,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else if drifted_count == 0 {
        println!(
            "Active: '{}' globally. No drift detected — all managed files match their namespace sources.",
            state.active_namespace
        );
    } else {
        println!("Active: '{}' globally.", state.active_namespace);
        for f in &files {
            match f.state.as_str() {
                "unchanged" => {}
                "modified" => println!("  [MODIFIED] ~/{}", f.path.display()),
                "missing" => println!("  [MISSING]  ~/{}", f.path.display()),
                "unexpected" => println!("  [UNEXPECTED] ~/{}", f.path.display()),
                other => println!("  [{other}] ~/{}", f.path.display()),
            }
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
    } else if added.is_empty() && removed.is_empty() && changed.is_empty() {
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
    Ok(())
}

struct DriftFile {
    path: PathBuf,
    state: String,
}
