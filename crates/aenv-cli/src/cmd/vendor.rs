//! `aenv vendor <source> --ns <ns> --path <subtree> --as <dest> [--pin <ref>]
//! [--adapter <a>] [--force]` — copy non-skill content (agents, commands,
//! reference docs, …) from a git source or local path into a namespace's tree,
//! declare it under the right adapter's `files`, and record provenance in a
//! `[[vendored]]` manifest entry (issue #2).
//!
//! Vendored files are ordinary authored content under `files`, so activation
//! and the resolver are unchanged — this is purely a manifest-authoring command.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use aenv_core::adapter::adapter_for_path;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::{FileKind, Filesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::{AdapterEntry, AenvManifest, VendoredDecl};
use aenv_core::skills::git::git_clone;

use crate::cmd::global::import::{looks_like_git_url, resolve_local_source, strip_git_prefix};

#[allow(clippy::too_many_arguments)]
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    namespace: &str,
    source: &str,
    src_path: &str,
    dest: &str,
    pin: Option<&str>,
    adapter_arg: Option<&str>,
    force: bool,
) -> Result<()> {
    // 1. Load the manifest.
    let manifest_path = layout.manifest_path(namespace);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(namespace.to_string()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("manifest not utf-8: {e}")))?;
    let mut manifest = AenvManifest::from_toml(text)?;
    let ns_dir = layout.namespace_dir(namespace);

    // 2. Which adapter owns the destination?
    let adapter_name = adapter_arg
        .map(str::to_string)
        .unwrap_or_else(|| adapter_for_path(dest).to_string());

    // 3. Fetch the source once. Git → throwaway sparse clone of `src_path`;
    //    local path → walked in place.
    let _tmp;
    let (tree_root, resolved_ref): (PathBuf, Option<String>) = if looks_like_git_url(source) {
        let url = strip_git_prefix(source);
        eprintln!(
            "Resolving {source}{}...",
            pin.map(|p| format!(" @ {p}")).unwrap_or_default()
        );
        let tmp = tempfile::tempdir().map_err(AenvError::Io)?;
        let clone_dir = tmp.path().join("clone");
        let sha = git_clone(url, pin, &clone_dir, Some(src_path))?;
        _tmp = Some(tmp);
        (clone_dir, Some(sha))
    } else {
        if pin.is_some() {
            return Err(AenvError::ManifestInvalid(
                "--pin only applies to git URL sources".into(),
            ));
        }
        _tmp = None;
        (resolve_local_source(source)?, None)
    };

    // 4. The subtree/file must exist at the resolved source.
    let src_abs = tree_root.join(src_path);
    if !fs.exists(&src_abs)? {
        return Err(AenvError::ManifestInvalid(format!(
            "source path '{src_path}' not found at {source}"
        )));
    }

    // 5. Plan the copy: (namespace-relative dest file, absolute source file).
    //    A file source maps to `dest`; a directory maps each file under it to
    //    `dest/<rel>`.
    let mut planned: Vec<(String, PathBuf)> = Vec::new();
    if fs.metadata(&src_abs)?.kind == FileKind::Directory {
        let mut rels: Vec<PathBuf> = Vec::new();
        collect_files(fs, &src_abs, Path::new(""), &mut rels)?;
        for rel in rels {
            let dest_file = join_rel(dest, &rel);
            planned.push((dest_file, src_abs.join(&rel)));
        }
    } else {
        planned.push((dest.to_string(), src_abs.clone()));
    }
    if planned.is_empty() {
        return Err(AenvError::ManifestInvalid(format!(
            "source path '{src_path}' contains no files to vendor"
        )));
    }
    planned.sort();

    // 6. Collision guard. A destination that already exists is an error unless
    //    it's owned by this same `[[vendored]]` entry (a re-vendor refresh) or
    //    `--force` is set. Check ALL before writing anything.
    let existing_idx = manifest
        .vendored
        .iter()
        .position(|v| v.source == source && v.src_path == src_path && v.dest == dest);
    let owned: BTreeSet<String> = existing_idx
        .map(|i| manifest.vendored[i].files.iter().cloned().collect())
        .unwrap_or_default();
    if !force {
        for (dest_file, _) in &planned {
            if fs.exists(&ns_dir.join(dest_file))? && !owned.contains(dest_file) {
                return Err(AenvError::ManifestInvalid(format!(
                    "destination '{dest_file}' already exists in namespace '{namespace}'; \
                     pass --force to overwrite"
                )));
            }
        }
    }

    // 7. Copy (resolving symlinks to their content) + record drift.
    let mut written: Vec<String> = Vec::new();
    let mut changed: Vec<String> = Vec::new();
    for (dest_file, src_file) in &planned {
        let new_bytes = fs.read(src_file)?;
        let abs = ns_dir.join(dest_file);
        let drifted = !fs.exists(&abs)? || fs.read(&abs)? != new_bytes;
        fs.write(&abs, &new_bytes)?;
        written.push(dest_file.clone());
        if drifted {
            changed.push(dest_file.clone());
        }
    }
    written.sort();
    written.dedup();

    // 8. Remove files this entry used to own that the source no longer provides.
    let written_set: BTreeSet<&String> = written.iter().collect();
    let mut removed: Vec<String> = owned
        .iter()
        .filter(|f| !written_set.contains(f))
        .cloned()
        .collect();
    removed.sort();
    for f in &removed {
        let _ = std::fs::remove_file(ns_dir.join(f));
    }

    // 9. Update the adapter's `files` (add written, drop removed; sorted+deduped).
    let entry = manifest
        .adapters
        .entry(adapter_name.clone())
        .or_insert_with(AdapterEntry::default);
    entry.files.retain(|f| !removed.contains(f));
    entry.files.extend(written.iter().cloned());
    entry.files.sort();
    entry.files.dedup();

    // 10. Upsert the `[[vendored]]` provenance entry.
    let decl = VendoredDecl {
        source: source.to_string(),
        ref_: resolved_ref.clone(),
        src_path: src_path.to_string(),
        dest: dest.to_string(),
        files: written.clone(),
    };
    match existing_idx {
        Some(i) => manifest.vendored[i] = decl,
        None => manifest.vendored.push(decl),
    }

    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;

    // 11. Report.
    let ref_disp = resolved_ref.as_deref().unwrap_or("local source");
    let verb = if existing_idx.is_some() {
        "Re-vendored"
    } else {
        "Vendored"
    };
    println!(
        "{verb} {} file{} from {source}@{ref_disp}:{src_path} into '{namespace}':{dest}",
        written.len(),
        if written.len() == 1 { "" } else { "s" }
    );
    for f in &written {
        let tag = if changed.contains(f) { "+" } else { "=" };
        println!("  {tag} {f}");
    }
    for f in &removed {
        println!("  - {f} (removed; no longer in source)");
    }
    Ok(())
}

/// Recursively collect file paths under `root`, as paths relative to `root`.
/// Follows symlinks (via `metadata`) so a symlinked file/dir contributes its
/// target's content.
fn collect_files<F: Filesystem>(
    fs: &F,
    root: &Path,
    rel: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    let abs = root.join(rel);
    match fs.metadata(&abs)?.kind {
        FileKind::File => {
            if !rel.as_os_str().is_empty() {
                out.push(rel.to_path_buf());
            }
        }
        FileKind::Directory => {
            let mut entries = fs.list_dir(&abs)?;
            entries.sort();
            for entry in entries {
                if let Some(name) = entry.file_name() {
                    collect_files(fs, root, &rel.join(name), out)?;
                }
            }
        }
        FileKind::Symlink => {} // metadata() follows links, so unreachable
    }
    Ok(())
}

/// Join a namespace-relative `dest` prefix with a `rel` path under it, as a
/// forward-slash string (manifest `files` paths are always `/`-separated).
fn join_rel(dest: &str, rel: &Path) -> String {
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    if rel_str.is_empty() {
        dest.to_string()
    } else {
        format!("{}/{}", dest.trim_end_matches('/'), rel_str)
    }
}
