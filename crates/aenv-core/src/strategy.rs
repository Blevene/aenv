//! Strategy selection: given the candidate list for a single path, decide
//! the materialization strategy.
//!
//! Priority order:
//!   1. Single candidate     -> Symlink (Identical decided at activation time)
//!   2. Manifest override    -> the named strategy on the latest candidate wins
//!   3. Adapter role         -> "instructions" => SectionMerge
//!   4. Adapter default_merge -> "deep" => DeepMerge(format-from-extension)
//!   5. Fallback              -> last-wins (Symlink to latest, earlier become shadows)

use std::path::Path;

use crate::adapter::AdapterRegistry;
use crate::resolve::{Candidate, DeepMergeFormat, MaterializeStrategy};
use crate::AenvError;

/// Decide the materialization strategy for a set of candidates for a single path.
///
/// The decision tree follows a priority order:
/// 1. Single candidate → `Symlink`
/// 2. Manifest override on the latest candidate → use the specified strategy
/// 3. Adapter role ("instructions") → `SectionMerge`
/// 4. Adapter default_merge ("deep") → `DeepMerge(format-from-extension)`
/// 5. Fallback → `Symlink` (last-wins)
pub fn decide_strategy(
    candidates: &[Candidate],
    adapters: &AdapterRegistry,
) -> Result<MaterializeStrategy, AenvError> {
    if candidates.is_empty() {
        return Err(AenvError::ManifestInvalid(
            "strategy selection called with no candidates".into(),
        ));
    }
    if candidates.len() == 1 {
        return Ok(MaterializeStrategy::Symlink);
    }

    let latest = candidates.last().unwrap();
    let path = latest.path.as_path();

    if let Some(name) = &latest.merge_override {
        return strategy_from_name(name, path);
    }

    if let Some(adapter) = adapters.get(&latest.adapter) {
        let path_key = path.to_string_lossy().to_string();
        if let Some(role) = adapter.roles.get(&path_key) {
            if role == "instructions" {
                return Ok(MaterializeStrategy::SectionMerge);
            }
        }
        if let Some(strat) = adapter.default_merge.get(&path_key) {
            return strategy_from_name(strat, path);
        }
    }

    Ok(MaterializeStrategy::Symlink)
}

fn strategy_from_name(name: &str, path: &Path) -> Result<MaterializeStrategy, AenvError> {
    match name {
        "section" | "section-merge" => Ok(MaterializeStrategy::SectionMerge),
        "deep" | "deep-merge" => Ok(MaterializeStrategy::DeepMerge(format_from_path(path)?)),
        "symlink" | "last-wins" => Ok(MaterializeStrategy::Symlink),
        other => Err(AenvError::ManifestInvalid(format!(
            "unknown merge strategy {other:?}; expected one of section, deep, last-wins"
        ))),
    }
}

fn format_from_path(path: &Path) -> Result<DeepMergeFormat, AenvError> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("json") => Ok(DeepMergeFormat::Json),
        Some("yaml" | "yml") => Ok(DeepMergeFormat::Yaml),
        Some("toml") => Ok(DeepMergeFormat::Toml),
        _ => Err(AenvError::ManifestInvalid(format!(
            "deep-merge requires .json, .yaml, .yml, or .toml extension; got {}",
            path.display()
        ))),
    }
}
