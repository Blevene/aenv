//! `aenv global diff` — placeholder until Milestone E Task 17.

use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(
    _fs: &F,
    _layout: &RegistryLayout,
    _fake_home: &Path,
    _ns_a: Option<&str>,
    _ns_b: Option<&str>,
    _json: bool,
) -> Result<()> {
    Err(AenvError::ManifestInvalid(
        "aenv global diff: not yet implemented (Task 17)".into(),
    ))
}
