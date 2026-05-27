//! User-scope snapshot — captures every adapter-managed path that currently
//! exists under `$HOME` into a new namespace.
//!
//! The dual of `aenv global activate`: instead of materializing namespace
//! contents into `$HOME`, this reads `$HOME` and materializes a namespace
//! recipe for re-playing the current state later.
//!
//! Designed for the "I have an existing `~/.claude/` set up by hand and want
//! to make it the seed of a namespace I can switch off and back on" flow.
//! The resulting namespace is byte-identical when re-activated, so the
//! materialization strategy on a round-trip is `Identical`.

use crate::adapter::AdapterRegistry;
use crate::error::{AenvError, Result};
use crate::fs::{FileKind, Filesystem};
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::manifest::{AdapterEntry, AenvManifest};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Summary returned by [`snapshot_global`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotSummary {
    /// Number of regular files copied (including files discovered inside
    /// recursively-snapshotted directories).
    pub files_copied: usize,
    /// Number of top-level directories captured (each contributes one entry
    /// to `user_files_declared`; the files inside are folded into
    /// `files_copied`).
    pub directories_copied: usize,
    /// Adapter-relative target paths that were actually captured (existed at
    /// snapshot time and made it into the manifest). Sorted, de-duplicated.
    pub user_files_declared: Vec<String>,
}

/// Strip a leading `~/` from a user-scope path declaration so it becomes
/// target-relative (i.e. relative to `$HOME`).
fn strip_tilde(s: &str) -> &str {
    s.strip_prefix("~/").unwrap_or(s)
}

/// Snapshot every adapter-managed user-scope path that exists under
/// `target_root` into a new namespace at `<layout>/envs/<name>/`.
///
/// - `name` must be a valid `NamespaceId` and the namespace dir must not
///   yet exist (fails with `ActivationConflict` if it does).
/// - `target_root` is the activation target (the CLI passes `$HOME`).
/// - `extra_includes` adds paths (relative to `target_root`) beyond every
///   installed adapter's declared `user_files` + `user_skills_dir`. They
///   may overlap with adapter paths; duplicates de-dupe.
///
/// On success, returns a [`SnapshotSummary`] describing what was captured.
/// All captured paths are attributed to the `claude-code` adapter in the
/// v0.1.0 contract — multi-adapter attribution by prefix is a future
/// enhancement (see plan F1).
pub fn snapshot_global<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    target_root: &Path,
    name: &str,
    extra_includes: &[String],
) -> Result<SnapshotSummary> {
    // 1. Validate name + namespace freshness.
    let _ = NamespaceId::new(name)?;
    let ns_dir = layout.namespace_dir(name);
    if fs.exists(&ns_dir)? {
        return Err(AenvError::ActivationConflict(format!(
            "namespace '{name}' already exists at {}; choose a different name",
            ns_dir.display()
        )));
    }

    // 2. Compute the candidate set (target-relative paths to consider).
    let mut candidates: BTreeSet<String> = BTreeSet::new();
    for (_name, adapter) in adapters.iter() {
        for raw in &adapter.user_files {
            let rel = strip_tilde(raw).trim_end_matches('/');
            if !rel.is_empty() {
                // Preserve the trailing slash for directory-marker entries
                // so the manifest re-emits them in their canonical "this is
                // a directory" form. Snapshot capture itself doesn't care
                // (it inspects the on-disk kind), but the manifest does.
                if raw.ends_with('/') {
                    candidates.insert(format!("{rel}/"));
                } else {
                    candidates.insert(rel.to_string());
                }
            }
        }
        if let Some(skills) = adapter.user_skills_dir.as_ref() {
            let rel = strip_tilde(skills).trim_end_matches('/');
            if !rel.is_empty() {
                candidates.insert(format!("{rel}/"));
            }
        }
    }
    for extra in extra_includes {
        let rel = extra.trim_start_matches('/').trim_end_matches('/');
        if !rel.is_empty() {
            candidates.insert(rel.to_string());
        }
    }

    // 3. For each candidate, capture into envs/<name>/user/<rel>.
    let user_root = ns_dir.join("user");
    let mut summary = SnapshotSummary::default();
    let mut captured: Vec<String> = Vec::new();

    for cand in &candidates {
        let lookup_rel = cand.trim_end_matches('/');
        let src = target_root.join(lookup_rel);
        if !fs.exists(&src)? {
            continue;
        }
        let kind = fs.symlink_metadata(&src)?.kind;
        let dst = user_root.join(lookup_rel);
        match kind {
            FileKind::File | FileKind::Symlink => {
                // Symlinks: capture resolved content as a regular file.
                let bytes = fs.read(&src)?;
                fs.write(&dst, &bytes)?;
                summary.files_copied += 1;
                captured.push(lookup_rel.to_string());
            }
            FileKind::Directory => {
                // Recursive copy is bounded by the contents — we don't fold
                // its individual files into `files_copied`; the directory
                // itself counts as one "directory captured" unit.
                let _copied = copy_dir_all(fs, &src, &dst)?;
                summary.directories_copied += 1;
                // Preserve the directory-marker form ("foo/") if the candidate
                // had one; otherwise record the bare path. The activate side
                // accepts both for trailing-slash-trimmed entries.
                let suffix = if cand.ends_with('/') { "/" } else { "" };
                captured.push(format!("{lookup_rel}{suffix}"));
            }
        }
    }

    captured.sort();
    captured.dedup();

    // 4. Write the manifest. Even an empty capture yields a valid (empty)
    //    namespace — that matches the "no-op snapshot is still a snapshot"
    //    expectation; we report 0/0 to the CLI which can decide whether to
    //    surface a hint.
    let mut adapters_block: BTreeMap<String, AdapterEntry> = BTreeMap::new();
    if !captured.is_empty() {
        adapters_block.insert(
            "claude-code".to_string(),
            AdapterEntry {
                files: Vec::new(),
                merge: None,
                user_files: captured.clone(),
                user_merge: None,
            },
        );
    }
    let manifest = AenvManifest {
        name: name.to_string(),
        extends: Vec::new(),
        adapters: adapters_block,
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        skills: Vec::new(),
    };
    let body =
        toml::to_string_pretty(&manifest).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    fs.write(&layout.manifest_path(name), body.as_bytes())?;

    summary.user_files_declared = captured;
    Ok(summary)
}

/// Recursively copy `src` into `dst`, returning the count of regular files
/// written. Symlinks are dereferenced — the destination receives the
/// resolved content as a regular file, matching the "capture bytes, not
/// identity" convention.
fn copy_dir_all<F: Filesystem>(fs: &F, src: &Path, dst: &Path) -> Result<usize> {
    let mut count = 0usize;
    let mut entries = fs.list_dir(src)?;
    entries.sort();
    fs.create_dir_all(dst)?;
    for entry in entries {
        let file_name = match entry.file_name() {
            Some(n) => n.to_os_string(),
            None => continue,
        };
        let dst_path = dst.join(PathBuf::from(&file_name));
        // Use symlink_metadata so a symlinked child shows up as Symlink,
        // letting us deref via `read` (which follows symlinks).
        let meta = fs.symlink_metadata(&entry)?;
        match meta.kind {
            FileKind::Directory => {
                count += copy_dir_all(fs, &entry, &dst_path)?;
            }
            FileKind::File | FileKind::Symlink => {
                let bytes = fs.read(&entry)?;
                fs.write(&dst_path, &bytes)?;
                count += 1;
            }
        }
    }
    Ok(count)
}
