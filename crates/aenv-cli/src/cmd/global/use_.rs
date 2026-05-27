//! `aenv global use <ns>` — placeholder until Milestone E Task 14.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(
    _fs: &F,
    _layout: &RegistryLayout,
    _adapters: &AdapterRegistry,
    _fake_home: &Path,
    _name: &str,
) -> Result<()> {
    Err(AenvError::ManifestInvalid(
        "aenv global use: not yet implemented (Task 14)".into(),
    ))
}
