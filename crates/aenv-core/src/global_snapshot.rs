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
//!
//! ## Importing from external sources
//!
//! This module also hosts [`import_global`], which is the dual-of-the-dual:
//! instead of reading `$HOME`, it reads a source directory tree (typically a
//! cloned git repo) and produces a namespace from it. The same dir-copying +
//! manifest-writing primitives are shared between the two flows.

use crate::adapter::AdapterRegistry;
use crate::error::{AenvError, Result};
use crate::fs::{FileKind, Filesystem};
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::manifest::{AdapterEntry, AenvManifest, LifecycleHooks};
use serde::Deserialize;
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
        let raw_kind = fs.symlink_metadata(&src)?.kind;
        // A symlink could point at a file or a directory. Resolve to the
        // target's kind so we don't try `read()` on a symlinked dir
        // (kernel returns EISDIR). Broken symlinks are skipped with a warn.
        let kind = match raw_kind {
            FileKind::Symlink => match fs.metadata(&src) {
                Ok(m) => Some(m.kind),
                Err(_) => None,
            },
            other => Some(other),
        };
        let dst = user_root.join(lookup_rel);
        match kind {
            Some(FileKind::File) | Some(FileKind::Symlink) => {
                // File (or symlink-of-symlink edge case): capture bytes.
                let bytes = fs.read(&src)?;
                fs.write(&dst, &bytes)?;
                summary.files_copied += 1;
                captured.push(lookup_rel.to_string());
            }
            Some(FileKind::Directory) => {
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
            None => {
                eprintln!(
                    "warning: skipping broken symlink at {} during snapshot",
                    src.display()
                );
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
                materialize: None,
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
        lifecycle: LifecycleHooks::default(),
    };
    let body =
        toml::to_string_pretty(&manifest).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    fs.write(&layout.manifest_path(name), body.as_bytes())?;

    summary.user_files_declared = captured;
    Ok(summary)
}

/// Summary returned by [`scaffold_global_namespace`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScaffoldSummary {
    /// Adapter-relative target paths declared in the generated manifest and
    /// scaffolded on disk under `user/`.
    pub user_files_declared: Vec<String>,
    /// The instructions file seeded with a starter header, if one was found.
    pub seeded_instructions: Option<String>,
}

/// Scaffold an empty, editable user-scope namespace from scratch.
///
/// The dual of `aenv create` for the global scope: instead of populating a
/// namespace from `$HOME` (snapshot) or an external tree (import), this seeds
/// a minimal hand-authorable starting point. It picks the adapter's
/// instructions-role user file (e.g. `~/.claude/CLAUDE.md` for claude-code),
/// writes it under `user/` with a one-line starter header, and declares it in
/// the manifest's `[adapters.<adapter>].user_files`. The result is
/// immediately `aenv global use`-able and ready to edit.
///
/// - `name` must be a valid `NamespaceId`; the namespace dir must not exist.
/// - `adapter_name` must be installed (else `AdapterMissing`).
pub fn scaffold_global_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    name: &str,
    adapter_name: &str,
) -> Result<ScaffoldSummary> {
    let _ = NamespaceId::new(name)?;
    let ns_dir = layout.namespace_dir(name);
    if fs.exists(&ns_dir)? {
        return Err(AenvError::ActivationConflict(format!(
            "namespace '{name}' already exists at {}; choose a different name",
            ns_dir.display()
        )));
    }
    let adapter = adapters
        .get(adapter_name)
        .ok_or_else(|| AenvError::AdapterMissing(adapter_name.to_string()))?;

    // Pick the path to seed: the adapter's instructions-role user file if it
    // declares one, else the first concrete (non-glob, non-directory) entry in
    // its `user_files`. Directory markers and globs aren't seedable as a single
    // editable file, so we skip them.
    let seed_raw = adapter
        .user_roles
        .iter()
        .find(|(_, role)| role.as_str() == "instructions")
        .map(|(p, _)| p.clone())
        .or_else(|| {
            adapter
                .user_files
                .iter()
                .find(|f| !f.contains('*') && !f.ends_with('/'))
                .cloned()
        });

    let mut summary = ScaffoldSummary::default();
    if let Some(raw) = seed_raw {
        let rel = strip_tilde(&raw).trim_end_matches('/').to_string();
        if !rel.is_empty() {
            let dst = ns_dir.join("user").join(&rel);
            fs.write(&dst, format!("# {name}\n").as_bytes())?;
            summary.user_files_declared.push(rel.clone());
            summary.seeded_instructions = Some(rel);
        }
    }

    let mut adapters_block: BTreeMap<String, AdapterEntry> = BTreeMap::new();
    adapters_block.insert(
        adapter_name.to_string(),
        AdapterEntry {
            files: Vec::new(),
            merge: None,
            user_files: summary.user_files_declared.clone(),
            user_merge: None,
            materialize: None,
        },
    );
    let manifest = AenvManifest {
        name: name.to_string(),
        extends: Vec::new(),
        adapters: adapters_block,
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
        skills: Vec::new(),
        lifecycle: LifecycleHooks::default(),
    };
    let body =
        toml::to_string_pretty(&manifest).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    fs.write(&layout.manifest_path(name), body.as_bytes())?;

    Ok(summary)
}

/// Parsed shape of a source repo's `aenv-namespace.toml` convention file.
///
/// All fields are optional; an empty convention file is legal. See
/// `pm_docs/aenv-namespace-toml-spec.md` for the full specification.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NamespaceImportSpec {
    /// Names of adapters this namespace touches. Each must already be
    /// installed at `<layout>/adapters/<name>.toml` (or be a builtin).
    /// Informational today; reserved for future validation.
    #[serde(default)]
    pub adapters: Vec<String>,
    /// Optional lifecycle hook paths. Path values are relative to the
    /// namespace dir (NOT to the source repo root, even though the import
    /// reads them from there) — the importer copies them in and the manifest
    /// stores the namespace-relative form.
    #[serde(default)]
    pub lifecycle: ImportLifecycleSpec,
    /// Source-path -> target-path map. Keys are source-relative paths;
    /// values are paths under `envs/<name>/user/`. Trailing `/` means
    /// directory.
    #[serde(default)]
    pub layout: BTreeMap<String, String>,
    /// Paths within the source tree to NOT copy (docs, dev artifacts).
    /// Matched against source-relative paths; supports trailing `*` glob.
    #[serde(default)]
    pub ignore: Vec<String>,
}

/// Lifecycle hooks block parsed from an `aenv-namespace.toml`.
///
/// Mirrors the shape of `AenvManifest::lifecycle` so the importer can copy
/// values straight across — kept as a distinct type so future spec-only
/// fields (`pre_activate`, environment hooks, etc.) can land here without
/// affecting the manifest contract.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ImportLifecycleSpec {
    /// Script (path relative to the namespace dir) to run on activate.
    #[serde(default)]
    pub on_activate: Option<String>,
    /// Script (path relative to the namespace dir) to run on deactivate.
    #[serde(default)]
    pub on_deactivate: Option<String>,
}

/// Summary returned by [`import_global`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// Number of regular files copied.
    pub files_copied: usize,
    /// Number of directories captured at the top level (their files are
    /// folded into `files_copied`).
    pub directories_copied: usize,
    /// Target paths (under `user/`) that ended up in the generated manifest.
    pub user_files_declared: Vec<String>,
    /// Whether an `aenv-namespace.toml` was present and used.
    pub convention_file_used: bool,
}

/// Build a namespace from a local source directory tree.
///
/// The importer first looks for `<source>/aenv-namespace.toml`. If present,
/// it's parsed as a [`NamespaceImportSpec`] and used as the authoritative
/// layout. If absent, a heuristic probes a fixed list of source-relative
/// paths (CLAUDE.md, agents/, hooks/, …) and maps them into the destination
/// namespace.
///
/// The generated manifest:
/// - lives at `envs/<name>/aenv.toml`
/// - declares each captured target path under the adapter whose prefix it
///   matches (`.claude/...` -> `claude-code`, `.codex/...` -> `codex`, else
///   `claude-code` as a fallback)
/// - declares a `[lifecycle]` block ONLY when a convention file
///   (`aenv-namespace.toml`) explicitly declares one. The heuristic never
///   infers lifecycle hooks from a repo's `install.sh` (see the
///   `heuristic_entries` helper).
pub fn import_global<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    _adapters: &AdapterRegistry,
    source: &Path,
    name: &str,
) -> Result<ImportSummary> {
    // 1. Validate name + namespace freshness.
    let _ = NamespaceId::new(name)?;
    let ns_dir = layout.namespace_dir(name);
    if fs.exists(&ns_dir)? {
        return Err(AenvError::ActivationConflict(format!(
            "namespace '{name}' already exists at {}; choose a different name",
            ns_dir.display()
        )));
    }

    // 2. Verify source exists and is a directory.
    if !fs.exists(source)? {
        return Err(AenvError::ManifestInvalid(format!(
            "import source '{}' does not exist",
            source.display()
        )));
    }
    let src_meta = fs.symlink_metadata(source)?;
    if src_meta.kind != FileKind::Directory {
        return Err(AenvError::ManifestInvalid(format!(
            "import source '{}' is not a directory",
            source.display()
        )));
    }

    // 3. Decide between convention-file vs heuristic.
    let conv_path = source.join("aenv-namespace.toml");
    let (entries, lifecycle, convention_file_used) = if fs.exists(&conv_path)? {
        let bytes = fs.read(&conv_path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| {
            AenvError::ManifestInvalid(format!(
                "aenv-namespace.toml at {} is not utf-8: {e}",
                conv_path.display()
            ))
        })?;
        let spec: NamespaceImportSpec = toml::from_str(text).map_err(|e| {
            AenvError::ManifestInvalid(format!(
                "aenv-namespace.toml at {} is malformed: {e}",
                conv_path.display()
            ))
        })?;
        let entries = layout_entries_from_spec(&spec);
        (entries, spec.lifecycle, true)
    } else {
        let (entries, lifecycle) = heuristic_entries(fs, source)?;
        (entries, lifecycle, false)
    };

    // 4. Copy each entry from source -> envs/<name>/user/<target>.
    let user_root = ns_dir.join("user");
    let mut summary = ImportSummary {
        convention_file_used,
        ..ImportSummary::default()
    };
    let mut captured: Vec<String> = Vec::new();

    for entry in &entries {
        let src_path = source.join(&entry.src_rel);
        if !fs.exists(&src_path)? {
            // Source path declared in spec but missing on disk — skip silently
            // so the same convention file works for partial subtrees.
            continue;
        }
        let kind = fs.symlink_metadata(&src_path)?.kind;
        let dst_rel = entry.target.trim_end_matches('/');
        let dst_path = user_root.join(dst_rel);
        match kind {
            FileKind::File | FileKind::Symlink => {
                let bytes = fs.read(&src_path)?;
                fs.write(&dst_path, &bytes)?;
                summary.files_copied += 1;
                captured.push(dst_rel.to_string());
            }
            FileKind::Directory => {
                copy_dir_all(fs, &src_path, &dst_path)?;
                summary.directories_copied += 1;
                let suffix = if entry.target.ends_with('/') { "/" } else { "" };
                captured.push(format!("{dst_rel}{suffix}"));
            }
        }
    }

    captured.sort();
    captured.dedup();

    // 5. Copy lifecycle scripts (if any) into the namespace dir root.
    //    These live alongside aenv.toml, not under user/, so the activator's
    //    Phase-K wiring can find them without going through the user-scope
    //    materialization path.
    for script in [
        lifecycle.on_activate.as_deref(),
        lifecycle.on_deactivate.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let script_src = source.join(script);
        if fs.exists(&script_src)? {
            let bytes = fs.read(&script_src)?;
            fs.write(&ns_dir.join(script), &bytes)?;
            summary.files_copied += 1;
        }
    }

    // 6. Bucket captured paths by adapter prefix.
    let mut claude_files: Vec<String> = Vec::new();
    let mut codex_files: Vec<String> = Vec::new();
    for p in &captured {
        if p.starts_with(".codex/") || p == ".codex" {
            codex_files.push(p.clone());
        } else {
            // `.claude/...` and the fallback bucket.
            claude_files.push(p.clone());
        }
    }

    // 7. Build the manifest. Even an empty capture yields a valid namespace
    //    (matches snapshot semantics).
    let mut adapters_block: BTreeMap<String, AdapterEntry> = BTreeMap::new();
    if !claude_files.is_empty() {
        adapters_block.insert(
            "claude-code".to_string(),
            AdapterEntry {
                files: Vec::new(),
                merge: None,
                user_files: claude_files,
                user_merge: None,
                materialize: None,
            },
        );
    }
    if !codex_files.is_empty() {
        adapters_block.insert(
            "codex".to_string(),
            AdapterEntry {
                files: Vec::new(),
                merge: None,
                user_files: codex_files,
                user_merge: None,
                materialize: None,
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
        lifecycle: LifecycleHooks {
            on_activate: lifecycle.on_activate.clone(),
            on_deactivate: lifecycle.on_deactivate.clone(),
        },
    };
    let body =
        toml::to_string_pretty(&manifest).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;

    fs.write(&layout.manifest_path(name), body.as_bytes())?;

    summary.user_files_declared = captured;
    Ok(summary)
}

/// One copy directive: where to read from in the source, where to write to
/// (relative to the namespace's `user/`).
#[derive(Debug, Clone)]
struct LayoutEntry {
    src_rel: String,
    target: String,
}

/// Translate a [`NamespaceImportSpec`] into a flat list of copy directives,
/// applying the `ignore` filter.
fn layout_entries_from_spec(spec: &NamespaceImportSpec) -> Vec<LayoutEntry> {
    let mut out = Vec::new();
    for (src, target) in &spec.layout {
        if ignore_matches(&spec.ignore, src) {
            continue;
        }
        out.push(LayoutEntry {
            src_rel: src.trim_end_matches('/').to_string(),
            target: target.clone(),
        });
    }
    out
}

/// Match a source-relative path against the `ignore` list. Supports:
/// - exact match (`"README.md"`)
/// - directory prefix with trailing slash (`"docs/"` matches `docs` and
///   anything under it)
/// - suffix glob with leading `*` (`"*.tmp"`)
/// - any-position glob with trailing `*` (`"docs/*"` matches anything under
///   `docs/`)
fn ignore_matches(ignore: &[String], path: &str) -> bool {
    let path_trim = path.trim_end_matches('/');
    for pat in ignore {
        if let Some(suffix) = pat.strip_prefix('*') {
            if path_trim.ends_with(suffix) {
                return true;
            }
            continue;
        }
        if let Some(prefix) = pat.strip_suffix('*') {
            let prefix = prefix.trim_end_matches('/');
            if path_trim == prefix || path_trim.starts_with(&format!("{prefix}/")) {
                return true;
            }
            continue;
        }
        let pat_trim = pat.trim_end_matches('/');
        if path_trim == pat_trim {
            return true;
        }
        if pat.ends_with('/') && path_trim.starts_with(&format!("{pat_trim}/")) {
            return true;
        }
    }
    false
}

/// Fallback path detection when no `aenv-namespace.toml` is present. Probes
/// a fixed list of well-known source-relative paths and maps them to
/// destination paths under `user/`.
///
/// Lifecycle hooks are deliberately NOT inferred here. A repository's
/// `install.sh` is, in practice, a self-installer that wants to own
/// `~/.claude` (validate a payload, back up the existing config, move itself
/// into place) — running it as an aenv `on_activate` fights aenv's own
/// materialization and stash. So the heuristic imports config files only;
/// lifecycle hooks must be declared explicitly in an `aenv-namespace.toml`,
/// where the author has opted into aenv's execution model (CWD, env vars,
/// rollback-on-failure) knowingly. See `pm_docs/aenv-namespace-toml-spec.md`.
fn heuristic_entries<F: Filesystem>(
    fs: &F,
    source: &Path,
) -> Result<(Vec<LayoutEntry>, ImportLifecycleSpec)> {
    // (source-relative, target-under-user). Trailing slash on target means
    // directory and is preserved into the manifest's user_files declaration.
    const PROBES: &[(&str, &str)] = &[
        ("CLAUDE.md", ".claude/CLAUDE.md"),
        ("AGENTS.md", ".codex/AGENTS.md"),
        ("settings.json", ".claude/settings.json"),
        ("agents/", ".claude/agents/"),
        ("commands/", ".claude/commands/"),
        ("hooks/", ".claude/hooks/"),
        ("skills/", ".claude/skills/"),
        ("runtime/", ".claude/runtime/"),
        ("bin/", ".claude/bin/"),
        ("sidecars/", ".claude/sidecars/"),
        (".codex/", ".codex/"),
    ];

    let mut entries = Vec::new();
    for (src_rel, target) in PROBES {
        let src_path = source.join(src_rel.trim_end_matches('/'));
        if fs.exists(&src_path)? {
            entries.push(LayoutEntry {
                src_rel: src_rel.trim_end_matches('/').to_string(),
                target: (*target).to_string(),
            });
        }
    }

    // Lifecycle hooks are opt-in via aenv-namespace.toml only — never inferred
    // from a repo's install.sh/uninstall.sh (see the doc comment above).
    Ok((entries, ImportLifecycleSpec::default()))
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
        // symlink_metadata so we can detect Symlink first; then resolve
        // the target's kind so a symlink → directory recurses correctly
        // instead of hitting EISDIR on read.
        let raw = fs.symlink_metadata(&entry)?;
        let kind = match raw.kind {
            FileKind::Symlink => match fs.metadata(&entry) {
                Ok(m) => Some(m.kind),
                Err(_) => None,
            },
            other => Some(other),
        };
        match kind {
            Some(FileKind::Directory) => {
                count += copy_dir_all(fs, &entry, &dst_path)?;
            }
            Some(FileKind::File) | Some(FileKind::Symlink) => {
                let bytes = fs.read(&entry)?;
                fs.write(&dst_path, &bytes)?;
                count += 1;
            }
            None => {
                eprintln!(
                    "warning: skipping broken symlink at {} during snapshot",
                    entry.display()
                );
            }
        }
    }
    Ok(count)
}
