//! Resolution output types.
//!
//! `ResolvedNamespace` is the product of walking the `extends` chain of a
//! leaf namespace. Every materializable artifact in the project carries a
//! `QualifiedName`, the strategy used to put it on disk, and (for shadowed
//! or merged artifacts) the qualified identities involved in the decision.
//!
//! Resolution itself lives in `resolve_namespace` (added in Task 3) — this
//! module owns only the data shapes.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::identity::{NamespaceId, QualifiedName};

/// The product of walking the `extends` chain of a leaf namespace.
///
/// Contains the full chain from root to leaf and the artifacts that should
/// be materialized in the project.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolvedNamespace {
    /// Root → leaf order. The leaf is the namespace the user pinned.
    pub chain: Vec<NamespaceId>,
    /// Ordered by materialized_path (lexicographic); this order is the
    /// activation order and the hashing order (Phase 5).
    pub artifacts: Vec<ResolvedArtifact>,
}

/// A single artifact that should be materialized in the project.
///
/// Carries the qualified name, the materialization strategy, and metadata
/// about any shadows (earlier-chain artifacts with the same path) or
/// contributors (for merged artifacts).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolvedArtifact {
    /// The qualified name (namespace::short-name) of this artifact.
    pub qualified_name: QualifiedName,
    /// The path where this artifact will be materialized in the project.
    pub materialized_path: PathBuf,
    /// The source path in the namespace directory.
    pub source_path: PathBuf,
    /// How this artifact should be materialized.
    pub strategy: MaterializeStrategy,
    /// Earlier-chain qualified names with the same short name + path.
    /// Empty for merged artifacts (every contributor is a co-producer, not a shadow).
    pub shadows: Vec<QualifiedName>,
    /// Ordered chain-of-contribution for merged artifacts. Empty otherwise.
    pub contributors: Vec<QualifiedName>,
}

/// How an artifact should be materialized in the project.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaterializeStrategy {
    /// Standard case: project file is a symlink to the namespace file.
    /// Serializes as `"symlink"` (matches Phase 1's lowercase form).
    Symlink,
    /// Project file already byte-identical to the namespace file — no symlink, no backup.
    Identical,
    /// Merged Markdown by `##` section.
    SectionMerge,
    /// Merged structured data in one of three formats.
    /// Serializes as `{"deep-merge": "json"}` etc.
    DeepMerge(DeepMergeFormat),
    /// Project file copied (Windows fallback, Phase 7); listed here for parity with state.rs.
    Copy,
    /// Phase 1 legacy variant. Accepted on read so old state files load; never
    /// emitted by Phase 2 code (which writes SectionMerge / DeepMerge instead).
    /// Phase 2's custom Deserialize for ManagedFile (Task 10) maps this to
    /// `SectionMerge` if encountered on a schema-1 state file.
    #[serde(rename = "merged")]
    Merged,
}

/// Serialization format for deep merge operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeepMergeFormat {
    /// JSON format.
    Json,
    /// YAML format.
    Yaml,
    /// TOML format.
    Toml,
}

// Note: Phase 1's `state.rs` defines its own `MaterializeStrategy` with
// `#[serde(rename_all = "lowercase")]`. Task 10 deletes that definition and
// re-exports this one. The kebab-case + alias combination above preserves
// schema-1 compatibility: existing on-disk state files store `"symlink"` and
// `"merged"`, both of which the new enum accepts.

// ---- Task 3: extends-chain resolver ----

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::adapter::AdapterRegistry;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::manifest::AenvManifest;
use crate::parameters::{resolve_parameters, ResolvedParameter};
use crate::policies::{resolve_policies, ResolvedPolicy};
use crate::state::SkillProvenance;
use crate::AenvError;

/// One candidate contribution from a single namespace for a single path.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Candidate {
    /// The namespace this candidate comes from.
    pub namespace: NamespaceId,
    /// Relative path of the artifact (project-relative).
    pub path: PathBuf,
    /// Absolute source path inside the namespace directory.
    pub source_path: PathBuf,
    /// Name of the adapter that manages this path.
    pub adapter: String,
    /// Per-file merge strategy override from the manifest, if any.
    pub merge_override: Option<String>,
    /// Skill provenance for skill SKILL.md files. `None` for regular files.
    pub skill_provenance: Option<SkillProvenance>,
}

/// Output of `resolve_namespace`.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ResolutionResult {
    /// Ordered chain from root ancestor to leaf (the namespace the user pinned).
    pub chain: Vec<NamespaceId>,
    /// All candidate artifacts gathered across the chain, in chain order.
    pub candidates: Vec<Candidate>,
    /// Effective parameters after `extends`-chain resolution.
    pub parameters: BTreeMap<String, ResolvedParameter>,
    /// Effective policies after `extends`-chain resolution.
    pub policies: BTreeMap<String, ResolvedPolicy>,
    /// Non-fatal warnings produced during resolution (e.g. an unrequired
    /// imported skill was unreachable and skipped). The library never prints
    /// these; the CLI consumes them after activation and emits to stderr.
    pub warnings: Vec<String>,
}

/// Errors specific to the resolution phase.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResolutionError {
    /// A cycle was detected in the `extends` graph. The vec is the offending sub-chain.
    Cycle(Vec<NamespaceId>),
    /// A referenced namespace does not exist in the registry.
    NamespaceNotFound(NamespaceId),
    /// A manifest references an adapter that is not installed.
    AdapterMissing(String),
    /// A manifest could not be parsed or failed a consistency check.
    ManifestInvalid {
        /// The namespace whose manifest failed.
        namespace: NamespaceId,
        /// Human-readable explanation of the failure.
        reason: String,
    },
    /// A required skill could not be resolved. Maps to exit 13.
    ActivationConflict(String),
    /// An I/O error occurred while reading the registry.
    Io(String),
}

impl From<ResolutionError> for AenvError {
    fn from(value: ResolutionError) -> Self {
        match value {
            ResolutionError::Cycle(chain) => {
                let rendered = chain
                    .iter()
                    .map(super::identity::NamespaceId::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                AenvError::ExtendsCycle(rendered)
            }
            ResolutionError::NamespaceNotFound(id) => {
                AenvError::NamespaceNotFound(id.as_str().to_owned())
            }
            ResolutionError::AdapterMissing(name) => AenvError::AdapterMissing(name),
            ResolutionError::ManifestInvalid { namespace, reason } => {
                AenvError::ManifestInvalid(format!("{namespace}: {reason}"))
            }
            ResolutionError::ActivationConflict(msg) => AenvError::ActivationConflict(msg),
            ResolutionError::Io(msg) => AenvError::Io(std::io::Error::other(msg)),
        }
    }
}

/// Walk the `extends` chain starting from `leaf`, returning the full ordered
/// chain (root → leaf) and all candidate artifacts gathered across it.
///
/// The walk is depth-first. Diamond inheritance is handled correctly: a node
/// reached via two parents appears exactly once, in left-branch-first position.
/// Cycles surface as `ResolutionError::Cycle`.
pub fn resolve_namespace<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    adapters: &AdapterRegistry,
    leaf: &NamespaceId,
) -> Result<ResolutionResult, ResolutionError> {
    let mut chain: Vec<NamespaceId> = Vec::new();
    let mut visiting: Vec<NamespaceId> = Vec::new();
    let mut visited: BTreeSet<NamespaceId> = BTreeSet::new();

    walk(fs, registry, leaf, &mut chain, &mut visiting, &mut visited)?;

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut params_per_ns: BTreeMap<
        NamespaceId,
        BTreeMap<String, crate::parameters::ParameterValue>,
    > = BTreeMap::new();
    let mut policies_per_ns: BTreeMap<NamespaceId, BTreeMap<String, crate::policies::PolicyDecl>> =
        BTreeMap::new();
    for ns in &chain {
        let manifest = load_manifest(fs, registry, ns)?;
        for adapter_name in manifest.adapters.keys() {
            if adapters.get(adapter_name).is_none() {
                return Err(ResolutionError::AdapterMissing(adapter_name.clone()));
            }
        }
        gather_candidates(fs, registry, ns, &manifest, &mut candidates)?;
        gather_skill_candidates(
            fs,
            registry,
            ns,
            &manifest,
            adapters,
            &mut candidates,
            &mut warnings,
        )?;
        params_per_ns.insert(ns.clone(), manifest.parameters.clone());
        policies_per_ns.insert(ns.clone(), manifest.policies.clone());
    }
    let parameters = resolve_parameters(&chain, &params_per_ns).map_err(|e| {
        ResolutionError::ManifestInvalid {
            namespace: leaf.clone(),
            reason: e.to_string(),
        }
    })?;
    let policies = resolve_policies(&chain, &policies_per_ns).map_err(|e| {
        ResolutionError::ManifestInvalid {
            namespace: leaf.clone(),
            reason: e.to_string(),
        }
    })?;
    crate::parameters::check_against_adapters(&parameters, adapters).map_err(|e| {
        ResolutionError::ManifestInvalid {
            namespace: leaf.clone(),
            reason: e.to_string(),
        }
    })?;
    Ok(ResolutionResult {
        chain,
        candidates,
        parameters,
        policies,
        warnings,
    })
}

fn walk<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    current: &NamespaceId,
    chain: &mut Vec<NamespaceId>,
    visiting: &mut Vec<NamespaceId>,
    visited: &mut BTreeSet<NamespaceId>,
) -> Result<(), ResolutionError> {
    if visited.contains(current) {
        return Ok(());
    }
    if visiting.contains(current) {
        let start = visiting.iter().position(|n| n == current).unwrap();
        let mut cycle: Vec<NamespaceId> = visiting[start..].to_vec();
        cycle.push(current.clone());
        return Err(ResolutionError::Cycle(cycle));
    }
    visiting.push(current.clone());
    let manifest = load_manifest(fs, registry, current)?;
    for parent in &manifest.extends {
        let parent_id =
            NamespaceId::new(parent.clone()).map_err(|e| ResolutionError::ManifestInvalid {
                namespace: current.clone(),
                reason: e.to_string(),
            })?;
        walk(fs, registry, &parent_id, chain, visiting, visited)?;
    }
    visiting.pop();
    visited.insert(current.clone());
    chain.push(current.clone());
    Ok(())
}

fn load_manifest<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    ns: &NamespaceId,
) -> Result<AenvManifest, ResolutionError> {
    let path = registry.manifest_path(ns.as_str());
    if !fs
        .exists(&path)
        .map_err(|e| ResolutionError::Io(e.to_string()))?
    {
        return Err(ResolutionError::NamespaceNotFound(ns.clone()));
    }
    let bytes = fs
        .read(&path)
        .map_err(|e| ResolutionError::Io(e.to_string()))?;
    let text = String::from_utf8(bytes).map_err(|e| ResolutionError::ManifestInvalid {
        namespace: ns.clone(),
        reason: format!("manifest is not valid UTF-8: {e}"),
    })?;
    let manifest: AenvManifest =
        AenvManifest::from_toml(&text).map_err(|e| ResolutionError::ManifestInvalid {
            namespace: ns.clone(),
            reason: e.to_string(),
        })?;
    if manifest.name != ns.as_str() {
        return Err(ResolutionError::ManifestInvalid {
            namespace: ns.clone(),
            reason: format!(
                "manifest name {:?} does not match directory name {:?}",
                manifest.name,
                ns.as_str()
            ),
        });
    }
    Ok(manifest)
}

fn gather_candidates<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    ns: &NamespaceId,
    manifest: &AenvManifest,
    out: &mut Vec<Candidate>,
) -> Result<(), ResolutionError> {
    let ns_root = registry.namespace_dir(ns.as_str());
    for (adapter_name, entry) in &manifest.adapters {
        for rel in &entry.files {
            if rel.contains('*') {
                expand_glob(fs, &ns_root, rel)
                    .map_err(|e| ResolutionError::Io(e.to_string()))?
                    .into_iter()
                    .for_each(|literal| {
                        out.push(Candidate {
                            namespace: ns.clone(),
                            path: PathBuf::from(&literal),
                            source_path: ns_root.join(&literal),
                            adapter: adapter_name.clone(),
                            merge_override: entry
                                .merge
                                .as_ref()
                                .and_then(|m| m.get(&literal).cloned()),
                            skill_provenance: None,
                        })
                    });
            } else {
                let source = ns_root.join(rel);
                if !fs
                    .exists(&source)
                    .map_err(|e| ResolutionError::Io(e.to_string()))?
                {
                    continue;
                }
                out.push(Candidate {
                    namespace: ns.clone(),
                    path: PathBuf::from(rel),
                    source_path: source,
                    adapter: adapter_name.clone(),
                    merge_override: entry.merge.as_ref().and_then(|m| m.get(rel).cloned()),
                    skill_provenance: None,
                });
            }
        }
    }
    Ok(())
}

/// Emit `Candidate`s for every skill file declared by this namespace.
///
/// Authored skills walk the namespace directory under
/// `<adapter.skills_dir>/<skill.name>/`. Imported skills resolve via
/// `apply_required_rule` and walk the resolved cache directory.
///
/// **Skill-name shadowing across the chain:** the caller invokes this
/// once per namespace in `extends` order (root → leaf). Two namespaces
/// declaring the same skill name produce two `Candidate`s with the same
/// `path` (e.g. both `.claude/skills/foo/SKILL.md`). The Phase 2
/// shadow/merge machinery in `activate::materialize_one` then groups
/// candidates by `path` and applies last-writer-wins (Symlink strategy),
/// so the leaf namespace's skill wins and the parent's is shadowed —
/// matching the behavior for regular adapter files.
fn gather_skill_candidates<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    ns: &NamespaceId,
    manifest: &AenvManifest,
    adapters: &AdapterRegistry,
    out: &mut Vec<Candidate>,
    warnings: &mut Vec<String>,
) -> Result<(), ResolutionError> {
    use crate::skills::SkillMode;

    for decl in &manifest.skills {
        // Determine adapter name for this skill.
        let adapter_name = match &decl.adapter {
            Some(a) => a.clone(),
            None => {
                if manifest.adapters.len() == 1 {
                    manifest.adapters.keys().next().unwrap().clone()
                } else {
                    return Err(ResolutionError::ManifestInvalid {
                        namespace: ns.clone(),
                        reason: format!(
                            "skill '{}' has no adapter and namespace has {} adapters; \
                             specify adapter explicitly",
                            decl.name,
                            manifest.adapters.len()
                        ),
                    });
                }
            }
        };

        let adapter = adapters
            .get(&adapter_name)
            .ok_or_else(|| ResolutionError::AdapterMissing(adapter_name.clone()))?;

        let skills_dir =
            adapter
                .skills_dir
                .as_deref()
                .ok_or_else(|| ResolutionError::ManifestInvalid {
                    namespace: ns.clone(),
                    reason: format!(
                        "adapter '{}' has no skills_dir; cannot materialize skill '{}'",
                        adapter_name, decl.name
                    ),
                })?;

        // Destination directory in project: <skills_dir>/<skill_name>/
        let dest_prefix = format!("{}/{}", skills_dir, decl.name);

        match decl.mode {
            SkillMode::Authored => {
                // Walk the namespace directory at <ns_root>/<dest_prefix>/
                let ns_root = registry.namespace_dir(ns.as_str());
                let skill_dir_abs = ns_root.join(&dest_prefix);
                if !fs
                    .exists(&skill_dir_abs)
                    .map_err(|e| ResolutionError::Io(e.to_string()))?
                {
                    // No skill directory present; skip silently.
                    continue;
                }
                // Walk all files under the skill directory.
                let mut rel_files: Vec<String> = Vec::new();
                walk_dir(
                    fs,
                    &ns_root,
                    std::path::Path::new(&dest_prefix),
                    &mut rel_files,
                )
                .map_err(|e| ResolutionError::Io(e.to_string()))?;
                for rel_str in rel_files {
                    let source_path = ns_root.join(&rel_str);
                    // Compute skill_provenance only for the SKILL.md file.
                    let skill_provenance = if std::path::Path::new(&rel_str).file_name()
                        == Some("SKILL.md".as_ref())
                    {
                        let bytes = fs
                            .read(&source_path)
                            .map_err(|e| ResolutionError::Io(e.to_string()))?;
                        let hash = crate::skills::cache::sha256_hex(&bytes);
                        Some(SkillProvenance {
                            source: format!("authored:{}", ns.as_str()),
                            resolved_ref: None,
                            resolved_hash: format!("sha256:{hash}"),
                        })
                    } else {
                        None
                    };
                    out.push(Candidate {
                        namespace: ns.clone(),
                        path: PathBuf::from(&rel_str),
                        source_path,
                        adapter: adapter_name.clone(),
                        merge_override: None,
                        skill_provenance,
                    });
                }
            }
            SkillMode::Imported => {
                match crate::skills::apply_required_rule(fs, registry, decl) {
                    Ok(Some(resolution)) => {
                        // Walk all files under resolution.source_path.
                        let source_dir = &resolution.source_path;
                        let mut rel_files: Vec<String> = Vec::new();
                        walk_dir(fs, source_dir, std::path::Path::new(""), &mut rel_files)
                            .map_err(|e| ResolutionError::Io(e.to_string()))?;
                        for rel_str in rel_files {
                            let source_path = source_dir.join(&rel_str);
                            // Attach provenance only to SKILL.md.
                            let skill_provenance = if std::path::Path::new(&rel_str).file_name()
                                == Some("SKILL.md".as_ref())
                            {
                                Some(SkillProvenance {
                                    source: decl
                                        .source
                                        .clone()
                                        .unwrap_or_else(|| "<unknown>".into()),
                                    resolved_ref: resolution.resolved_ref.clone(),
                                    resolved_hash: resolution.resolved_hash.clone(),
                                })
                            } else {
                                None
                            };
                            // Destination path: <skills_dir>/<skill_name>/<rel_str>
                            let dest_path = format!("{dest_prefix}/{rel_str}");
                            out.push(Candidate {
                                namespace: ns.clone(),
                                path: PathBuf::from(&dest_path),
                                source_path,
                                adapter: adapter_name.clone(),
                                merge_override: None,
                                skill_provenance,
                            });
                        }
                    }
                    Ok(None) => {
                        warnings.push(format!(
                            "skill '{}' from '{}' unreachable; skipping (not required)",
                            decl.name,
                            decl.source.as_deref().unwrap_or("<no source>")
                        ));
                    }
                    Err(e) => {
                        return Err(ResolutionError::ActivationConflict(format!(
                            "required skill '{}' unreachable: {}",
                            decl.name, e
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}

fn expand_glob<F: Filesystem>(
    fs: &F,
    ns_root: &std::path::Path,
    pattern: &str,
) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    walk_dir(fs, ns_root, std::path::Path::new(""), &mut out)?;
    Ok(out
        .into_iter()
        .filter(|rel| glob_match(pattern, rel))
        .collect())
}

fn walk_dir<F: Filesystem>(
    fs: &F,
    abs_base: &std::path::Path,
    rel_prefix: &std::path::Path,
    out: &mut Vec<String>,
) -> std::io::Result<()> {
    let abs = abs_base.join(rel_prefix);
    for entry in fs.list_dir(&abs)? {
        let name = match entry.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        let child_rel = rel_prefix.join(&name);
        let child_abs = abs_base.join(&child_rel);
        let meta = fs.metadata(&child_abs)?;
        if matches!(meta.kind, crate::fs::FileKind::Directory) {
            walk_dir(fs, abs_base, &child_rel, out)?;
        } else {
            out.push(child_rel.to_string_lossy().to_string());
        }
    }
    Ok(())
}

fn glob_match(pattern: &str, candidate: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**/*") {
        candidate.starts_with(prefix) && candidate[prefix.len()..].starts_with('/')
    } else if let Some(prefix) = pattern.strip_suffix("**/*") {
        candidate.starts_with(prefix)
    } else {
        pattern == candidate
    }
}
