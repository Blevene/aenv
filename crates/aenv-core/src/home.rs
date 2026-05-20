//! Registry-directory layout helpers.
//!
//! `RegistryLayout` is a thin wrapper around the absolute path to `AENV_HOME`
//! (default `~/.aenv`) that knows where namespaces, adapters, and config
//! files live underneath. The CLI layer is responsible for resolving the
//! `AENV_HOME` env var (or default) into an absolute path; this type takes
//! that absolute path and computes everything else from it.

use std::path::{Path, PathBuf};

/// Layout of the aenv registry directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryLayout {
    root: PathBuf,
}

impl RegistryLayout {
    /// Create a layout rooted at `root`. `root` must be absolute.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The registry root itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The `envs/` subdirectory holding all namespaces.
    pub fn namespaces_dir(&self) -> PathBuf {
        self.root.join("envs")
    }

    /// The directory containing the namespace named `name`.
    pub fn namespace_dir(&self, name: &str) -> PathBuf {
        self.namespaces_dir().join(name)
    }

    /// The manifest path (`aenv.toml`) for the namespace named `name`.
    pub fn manifest_path(&self, name: &str) -> PathBuf {
        self.namespace_dir(name).join("aenv.toml")
    }

    /// The `adapters/` subdirectory holding adapter TOML files.
    pub fn adapters_dir(&self) -> PathBuf {
        self.root.join("adapters")
    }

    /// The global config file (`config.toml`).
    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }
}
