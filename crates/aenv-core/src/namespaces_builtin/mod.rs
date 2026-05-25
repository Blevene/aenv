//! Built-in default namespaces embedded into the binary.
//!
//! Mirrors `adapters_builtin`: each namespace ships as a set of embedded
//! `include_str!` blobs and is written to `envs/<name>/` on first run. A
//! file that already exists is left untouched so user edits stick — and so
//! a user who deletes a default namespace only sees it reappear if they
//! also remove the directory.

use crate::error::Result;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;

const KARPATHY_AENV_TOML: &str = include_str!("karpathy/aenv.toml");
const KARPATHY_CLAUDE_MD: &str = include_str!("karpathy/CLAUDE.md");
const CHERNY_AENV_TOML: &str = include_str!("cherny/aenv.toml");
const CHERNY_CLAUDE_MD: &str = include_str!("cherny/CLAUDE.md");

const KARPATHY_FILES: &[(&str, &str)] = &[
    ("aenv.toml", KARPATHY_AENV_TOML),
    ("CLAUDE.md", KARPATHY_CLAUDE_MD),
];

const CHERNY_FILES: &[(&str, &str)] = &[
    ("aenv.toml", CHERNY_AENV_TOML),
    ("CLAUDE.md", CHERNY_CLAUDE_MD),
];

/// Every built-in namespace as a (name, files) pair, where `files` is a
/// list of (relative-path, contents) tuples written under `envs/<name>/`.
pub const ALL: &[(&str, &[(&str, &str)])] =
    &[("karpathy", KARPATHY_FILES), ("cherny", CHERNY_FILES)];

/// Write every built-in namespace under `layout.namespaces_dir()` if not
/// already present. A file is skipped if it already exists on disk — even
/// if its contents differ from the embedded version — so user edits stick.
pub fn ensure_written<F: Filesystem>(fs: &F, layout: &RegistryLayout) -> Result<()> {
    for (name, files) in ALL {
        let ns_dir = layout.namespace_dir(name);
        for (rel, body) in *files {
            let target = ns_dir.join(rel);
            if fs.exists(&target)? {
                continue;
            }
            fs.write(&target, body.as_bytes())?;
        }
    }
    Ok(())
}
