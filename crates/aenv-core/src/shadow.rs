//! Shadow-chain computation.
//!
//! When two or more candidates in a chain target the same path and the
//! resolved strategy is non-merge (Symlink/Identical/Copy), the latest
//! candidate is the "provided" artifact and the earlier candidates are
//! shadowed. For merge strategies (SectionMerge/DeepMerge), every
//! candidate is a contributor and the shadow set is empty.

use crate::adapter::AdapterRegistry;
use crate::identity::{QualifiedName, ShortName};
use crate::resolve::{Candidate, MaterializeStrategy};

/// Compute the shadow chain for a path given a list of candidates and strategy.
///
/// For non-merge strategies (Symlink, Copy, Identical, Merged), earlier
/// candidates in the chain are shadowed by the latest one. Returns their
/// QualifiedNames in root-to-near order.
///
/// For merge strategies (SectionMerge, DeepMerge), every candidate is a
/// contributor and the shadow set is empty.
///
/// Single-candidate paths have no shadows.
pub fn compute_shadows(
    candidates: &[Candidate],
    strategy: MaterializeStrategy,
    _adapters: &AdapterRegistry,
) -> crate::Result<Vec<QualifiedName>> {
    if candidates.len() < 2 {
        return Ok(Vec::new());
    }
    match strategy {
        MaterializeStrategy::SectionMerge | MaterializeStrategy::DeepMerge(_) => Ok(Vec::new()),
        MaterializeStrategy::Symlink
        | MaterializeStrategy::Copy
        | MaterializeStrategy::Identical
        | MaterializeStrategy::Merged => {
            // Everything except the last candidate is shadowed.
            candidates[..candidates.len() - 1]
                .iter()
                .map(qualified_from_candidate)
                .collect()
        }
    }
}

/// Compute the QualifiedName for a candidate's contribution.
///
/// Returns `Err` if the candidate's path contains the `::` separator (which
/// is invalid as a short name). A well-formed manifest can never declare such
/// a file, so this is effectively unreachable — but it surfaces as a clean
/// `ManifestInvalid` rather than a panic if someone hand-crafts a malicious
/// manifest.
pub(crate) fn qualified_from_candidate(c: &Candidate) -> crate::Result<QualifiedName> {
    let short = ShortName::new(c.path.to_string_lossy().to_string())?;
    Ok(QualifiedName::new(c.namespace.clone(), short))
}
