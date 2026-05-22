//! Registry-source stub. Phase 4 does not resolve registry sources —
//! the registry design is still pending (PRD §3 open question).

use crate::error::{AenvError, Result};
use crate::skills::local::ResolvedSkill;

/// Always returns `ManifestInvalid` with a clear "not yet implemented" message.
pub fn resolve_registry(name: &str, _ref_spec: Option<&str>) -> Result<ResolvedSkill> {
    Err(AenvError::ManifestInvalid(format!(
        "registry source 'registry:{name}' is not yet implemented \
         (pending PRD §3 registry design)"
    )))
}
