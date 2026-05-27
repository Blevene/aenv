//! `aenv global deactivate` — placeholder until Milestone E Task 15.

use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(_fs: &F, _layout: &RegistryLayout, _fake_home: &Path) -> Result<()> {
    Err(AenvError::ManifestInvalid(
        "aenv global deactivate: not yet implemented (Task 15)".into(),
    ))
}
