//! Pure material-set computation — the read-only counterpart to
//! `activate_namespace`.
//!
//! Returns the same `(project_relative_path, content_bytes)` pairs that
//! activation would write, without touching the project filesystem.
//! Section-merged and deep-merged artifacts are produced by the same
//! merge primitives activation uses; symlinked artifacts contribute the
//! source file's raw bytes. `Copy` and `Merged` strategies fail with
//! `ActivationConflict`, mirroring the hard errors activation produces.
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

/// Compute the material set for `leaf` without writing anything.
pub fn compute_material_set<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    leaf: &NamespaceId,
) -> Result<MaterialSet> {
    let resolution = resolve_namespace(fs, layout, adapters, leaf)?;

    // Project-scope only. User-scope candidates are part of the user-scope
    // material set (and the user-scope hash, Task 23); they are not included
    // here so the project-scope hash stays stable for namespaces that also
    // declare `user_files`.
    let mut by_path: BTreeMap<PathBuf, Vec<Candidate>> = BTreeMap::new();
    for c in resolution
        .candidates
        .into_iter()
        .filter(|c| c.scope == crate::scope::Scope::Project)
    {
        by_path.entry(c.path.clone()).or_default().push(c);
    }

    let mut entries: Vec<(PathBuf, Vec<u8>)> = Vec::with_capacity(by_path.len());
    for (path, candidates) in by_path {
        let strategy = decide_strategy(&candidates, adapters)?;
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
        MaterializeStrategy::Symlink | MaterializeStrategy::Identical => {
            let winner = candidates.last().expect("at least one candidate");
            fs.read(&winner.source_path).map_err(AenvError::from)
        }
        MaterializeStrategy::Copy => Err(AenvError::ActivationConflict(
            "Copy strategy is Phase 7 (Windows fallback); not supported in Phase 2".into(),
        )),
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
