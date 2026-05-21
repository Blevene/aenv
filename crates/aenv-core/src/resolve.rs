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
