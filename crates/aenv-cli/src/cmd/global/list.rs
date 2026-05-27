//! `aenv global list` — placeholder until Milestone E Task 17.

use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;

pub fn run<F: Filesystem>(_fs: &F, _layout: &RegistryLayout, _json: bool) -> Result<()> {
    Err(AenvError::ManifestInvalid(
        "aenv global list: not yet implemented (Task 17)".into(),
    ))
}
