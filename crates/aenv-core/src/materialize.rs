//! Pure material-set computation — the read-only counterpart to
//! `activate_namespace`.
//!
//! Returns the same `(project_relative_path, content_bytes)` pairs that
//! activation would write, without touching the project filesystem.
//! Section-merged and deep-merged artifacts are produced by the same
//! merge primitives activation uses; symlinked and copy-mode artifacts
//! contribute the source file's raw bytes. The `Merged` strategy fails
//! with `ActivationConflict` (Phase-1 sentinel never produced here).
//!
//! This is the input to `hash::hash_resolved_namespace`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::adapter::AdapterRegistry;
use crate::error::Result;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::parameters::ResolvedParameter;
use crate::resolve::{resolve_namespace, Candidate, DeepMergeFormat, MaterializeStrategy};
use crate::scope::Scope;
use crate::strategy::decide_strategy;
use crate::AenvError;

/// Output of `compute_material_set`. Entries are sorted by path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialSet {
    /// Sorted (project-relative path, post-merge bytes) pairs.
    ///
    /// Invariant: every path is relative and uses forward slashes only
    /// (cross-platform hash stability). Use [`MaterialSet::new`] to construct;
    /// use [`MaterialSet::entries`] to read. Direct mutation is intentionally
    /// prevented by the `pub(crate)` visibility.
    pub(crate) entries: Vec<(PathBuf, Vec<u8>)>,
    /// Resolved parameter map. Carried alongside so the hash function can
    /// append it as the synthetic `.aenv/parameters.json` entry.
    pub parameters: BTreeMap<String, ResolvedParameter>,
}

impl MaterialSet {
    /// Construct a `MaterialSet`, asserting that every path is relative and
    /// uses forward slashes (cross-platform hash stability invariant).
    ///
    /// The `debug_assert!`s fire only in debug builds; the cost vanishes in
    /// release while the contract remains documented and tested by the
    /// cross-machine fixture.
    pub fn new(
        entries: Vec<(PathBuf, Vec<u8>)>,
        parameters: BTreeMap<String, ResolvedParameter>,
    ) -> Self {
        for (path, _) in &entries {
            debug_assert!(
                path.is_relative(),
                "MaterialSet path must be relative: {}",
                path.display()
            );
            debug_assert!(
                !path.to_string_lossy().contains('\\'),
                "MaterialSet path must not contain backslashes: {} \
                 (use forward slashes for cross-platform hash stability)",
                path.display()
            );
        }
        Self {
            entries,
            parameters,
        }
    }

    /// Read-only view of the (path, content) pairs, in lex order.
    pub fn entries(&self) -> &[(PathBuf, Vec<u8>)] {
        &self.entries
    }
}

/// Compute the project-scope material set for `leaf` without writing anything.
///
/// User-scope candidates are not included here — see [`compute_material_set_user`]
/// for the symmetric variant. Keeping the two scopes apart guarantees the
/// project-scope hash stays stable for namespaces that also declare `user_files`
/// (R-84 + Issue #4 Milestone G invariant).
pub fn compute_material_set<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    leaf: &NamespaceId,
) -> Result<MaterialSet> {
    compute_material_set_for_scope(fs, layout, adapters, leaf, Scope::Project)
}

/// Compute the user-scope material set for `leaf` without writing anything.
///
/// Symmetric to [`compute_material_set`], but filters candidates to
/// [`Scope::User`] so the returned set covers only what `aenv global activate` would
/// materialize. Returns an empty entry list (with the resolved-parameter map
/// still populated) when the namespace has no user-scope candidates — that is
/// not an error.
pub fn compute_material_set_user<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    leaf: &NamespaceId,
) -> Result<MaterialSet> {
    compute_material_set_for_scope(fs, layout, adapters, leaf, Scope::User)
}

fn compute_material_set_for_scope<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    leaf: &NamespaceId,
    scope: Scope,
) -> Result<MaterialSet> {
    let resolution = resolve_namespace(fs, layout, adapters, leaf)?;

    let mut by_path: BTreeMap<PathBuf, Vec<Candidate>> = BTreeMap::new();
    for c in resolution
        .candidates
        .into_iter()
        .filter(|c| c.scope == scope)
    {
        by_path.entry(c.path.clone()).or_default().push(c);
    }

    let mut entries: Vec<(PathBuf, Vec<u8>)> = Vec::with_capacity(by_path.len());
    for (path, candidates) in by_path {
        let strategy = decide_strategy(&candidates, adapters)?;
        // A pass-through (`Symlink`/`Identical`/`Copy`) entry whose source is a
        // directory — e.g. a `user_files = [".claude/agents/"]` entry from a
        // heuristic import. Activation symlinks the directory as a unit, so its
        // resolved material is the directory's recursive file contents. Reading
        // the directory itself as bytes fails with `Is a directory`; expand it
        // into one entry per contained file so the hash covers the whole tree,
        // matching what the symlinked directory exposes on disk.
        if matches!(
            strategy,
            MaterializeStrategy::Symlink
                | MaterializeStrategy::Identical
                | MaterializeStrategy::Copy
        ) {
            let winner = candidates.last().expect("at least one candidate");
            let is_dir = fs
                .metadata(&winner.source_path)
                .map(|m| matches!(m.kind, crate::fs::FileKind::Directory))
                .unwrap_or(false);
            if is_dir {
                let mut rels = Vec::new();
                crate::resolve::walk_dir(
                    fs,
                    &winner.source_path,
                    std::path::Path::new(""),
                    &mut rels,
                )
                .map_err(AenvError::from)?;
                for rel in rels {
                    let bytes = fs
                        .read(&winner.source_path.join(&rel))
                        .map_err(AenvError::from)?;
                    entries.push((path.join(&rel), bytes));
                }
                continue;
            }
        }
        let bytes = materialize_one_in_memory(fs, &candidates, strategy)?;
        entries.push((path, bytes));
    }

    // Byte-wise lex sort on UTF-8 path (defensive — BTreeMap iteration
    // is already sorted, but the hash contract requires this exact order).
    entries.sort_by(|a, b| {
        a.0.as_os_str()
            .as_encoded_bytes()
            .cmp(b.0.as_os_str().as_encoded_bytes())
    });

    Ok(MaterialSet::new(entries, resolution.parameters))
}

fn materialize_one_in_memory<F: Filesystem>(
    fs: &F,
    candidates: &[Candidate],
    strategy: MaterializeStrategy,
) -> Result<Vec<u8>> {
    match strategy {
        MaterializeStrategy::Symlink
        | MaterializeStrategy::Identical
        | MaterializeStrategy::Copy => {
            // All three resolve to the source bytes of the winning candidate.
            // Symlink reads through the link; Identical's target is byte-equal
            // by definition; Copy's materialized bytes ARE the source bytes
            // (any later edits are drift, not part of the resolved set).
            let winner = candidates.last().expect("at least one candidate");
            fs.read(&winner.source_path).map_err(AenvError::from)
        }
        MaterializeStrategy::Merged => Err(AenvError::ActivationConflict(
            "Phase 1 'Merged' variant should not be produced by Phase 2".into(),
        )),
        MaterializeStrategy::SectionMerge => {
            let bodies = read_all_as_strings(fs, candidates)?;
            let merged = crate::merge::section::merge_sections(&bodies);
            Ok(merged.into_bytes())
        }
        MaterializeStrategy::DeepMerge(format) => {
            let bodies = read_all_as_bytes(fs, candidates)?;
            match format {
                DeepMergeFormat::Json => {
                    crate::merge::deep_json::merge_json(&bodies).map_err(AenvError::from)
                }
                DeepMergeFormat::Yaml => {
                    crate::merge::deep_yaml::merge_yaml(&bodies).map_err(AenvError::from)
                }
                DeepMergeFormat::Toml => {
                    crate::merge::deep_toml::merge_toml(&bodies).map_err(AenvError::from)
                }
            }
        }
    }
}

fn read_all_as_bytes<F: Filesystem>(fs: &F, candidates: &[Candidate]) -> Result<Vec<Vec<u8>>> {
    candidates
        .iter()
        .map(|c| fs.read(&c.source_path).map_err(AenvError::from))
        .collect()
}

fn read_all_as_strings<F: Filesystem>(fs: &F, candidates: &[Candidate]) -> Result<Vec<String>> {
    candidates
        .iter()
        .map(|c| {
            let bytes = fs.read(&c.source_path).map_err(AenvError::from)?;
            String::from_utf8(bytes).map_err(|e| {
                AenvError::ActivationConflict(format!(
                    "UTF-8 decode {}: {e}",
                    c.source_path.display()
                ))
            })
        })
        .collect()
}
