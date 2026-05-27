//! `aenv global doctor` — placeholder until Milestone E Task 19.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;

pub fn run<F: Filesystem>(
    _fs: &F,
    _layout: &RegistryLayout,
    _adapters: &AdapterRegistry,
    _namespace: Option<&str>,
    _json: bool,
) -> Result<()> {
    Err(AenvError::ManifestInvalid(
        "aenv global doctor: not yet implemented (Task 19)".into(),
    ))
}
