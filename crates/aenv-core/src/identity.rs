//! Namespace identity types — the wire format of `aenv`.
//!
//! `NamespaceId` and `ShortName` are validated newtypes; their `Display` impls
//! and `FromStr` parser define the `::`-separated qualified-name format used
//! in `.aenv-state/state.json`, machine output (Phase 5), and the `aenv which`
//! command. Changing any of this is a major-version break.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::AenvError;

const SEPARATOR: &str = "::";

/// A validated namespace identifier.
///
/// Namespace IDs are used as the first part of a qualified name (e.g., `base`
/// in `base::CLAUDE.md`). They must not be empty, contain colons, or match
/// the reserved `(merged)` synthetic namespace.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NamespaceId(String);

impl NamespaceId {
    /// Reserved synthetic namespace used by `activate_namespace` to label
    /// merged artifacts whose contributors span multiple real namespaces.
    /// Rejected from user-facing construction so a real namespace can never
    /// collide with the synthesizer's output.
    pub const RESERVED_MERGED: &'static str = "(merged)";

    /// Construct a new namespace ID, validating it for emptiness, colons, and
    /// reserved names.
    pub fn new(s: impl Into<String>) -> Result<Self, AenvError> {
        let s = s.into();
        if s.is_empty() {
            return Err(AenvError::ManifestInvalid(
                "namespace name cannot be empty".into(),
            ));
        }
        if s.contains(':') {
            return Err(AenvError::ManifestInvalid(format!(
                "namespace name {s:?} cannot contain ':'"
            )));
        }
        if s == Self::RESERVED_MERGED {
            return Err(AenvError::ManifestInvalid(format!(
                "namespace name {s:?} is reserved; aenv uses it internally to label \
                 merged artifacts in state.json and 'aenv which' output. Pick a \
                 different name (e.g. 'merged-base' or 'combined')."
            )));
        }
        Ok(Self(s))
    }

    /// Construct the reserved synthetic `(merged)` namespace. The single
    /// intended caller is `aenv-core::activate::synthesize_merged_qn`. Test
    /// helpers that need to construct a synthetic qualified name (e.g. test
    /// fixtures comparing against state.json output) also call this.
    pub fn merged_synthetic() -> Self {
        Self(Self::RESERVED_MERGED.to_owned())
    }

    /// Return the namespace ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated short name (the second part of a qualified name).
///
/// Short names can be paths (e.g., `.claude/skills/write-tests/SKILL.md`),
/// allowing slashes and dots. They must not be empty or contain the `::` separator.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShortName(String);

impl ShortName {
    /// Construct a new short name, validating it for emptiness and the `::` separator.
    pub fn new(s: impl Into<String>) -> Result<Self, AenvError> {
        let s = s.into();
        if s.is_empty() {
            return Err(AenvError::ManifestInvalid(
                "short name cannot be empty".into(),
            ));
        }
        if s.contains(SEPARATOR) {
            return Err(AenvError::ManifestInvalid(format!(
                "short name {s:?} cannot contain '::'"
            )));
        }
        Ok(Self(s))
    }

    /// Return the short name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ShortName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A qualified name combining a namespace ID and short name with `::` separator.
///
/// Qualified names are the identity of assets in aenv (e.g., `base::CLAUDE.md`
/// identifies a specific file in a specific namespace). They are used as keys
/// in state.json, in `aenv which` output, and in the public API.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct QualifiedName {
    namespace: NamespaceId,
    short: ShortName,
}

impl QualifiedName {
    /// Construct a qualified name from a namespace ID and short name.
    pub fn new(namespace: NamespaceId, short: ShortName) -> Self {
        Self { namespace, short }
    }

    /// Return the namespace part of this qualified name.
    pub fn namespace(&self) -> &NamespaceId {
        &self.namespace
    }

    /// Return the short name part of this qualified name.
    pub fn short(&self) -> &ShortName {
        &self.short
    }
}

impl fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.namespace, SEPARATOR, self.short)
    }
}

impl FromStr for QualifiedName {
    type Err = AenvError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Exactly one occurrence of `::` is the contract.
        let occurrences: Vec<_> = s.match_indices(SEPARATOR).collect();
        if occurrences.len() != 1 {
            return Err(AenvError::ManifestInvalid(format!(
                "qualified name {s:?} must contain exactly one '::' separator"
            )));
        }
        let (idx, _) = occurrences[0];
        let ns = &s[..idx];
        let short = &s[idx + SEPARATOR.len()..];
        Ok(Self {
            namespace: NamespaceId::new(ns)?,
            short: ShortName::new(short)?,
        })
    }
}

/// Serialize as the canonical `"namespace::short"` string form.
impl Serialize for QualifiedName {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// Deserialize from the canonical `"namespace::short"` string form.
impl<'de> Deserialize<'de> for QualifiedName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        raw.parse::<QualifiedName>().map_err(serde::de::Error::custom)
    }
}
