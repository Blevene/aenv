//! `aenv global import <source> [<name>]` — turn a directory tree or a git
//! URL into an aenv namespace.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::skills::git::git_clone;
use std::path::{Path, PathBuf};

/// Run the import command. Detects whether `source` is a git URL; if so,
/// clones (optionally pinning to `pin`) into a tempdir and re-enters with the
/// cloned dir as the local source. Otherwise treats `source` as a local path.
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    source: &str,
    name: &str,
    pin: Option<&str>,
) -> Result<()> {
    if looks_like_git_url(source) {
        return run_git(fs, layout, adapters, source, name, pin);
    }
    if pin.is_some() {
        return Err(AenvError::ManifestInvalid(
            "--pin only applies to git URL sources".into(),
        ));
    }

    let src_path = resolve_local_source(source)?;
    let effective_name = if name.is_empty() {
        default_name_for(&src_path)?
    } else {
        name.to_string()
    };

    let summary = aenv_core::global_snapshot::import_global(
        fs,
        layout,
        adapters,
        &src_path,
        &effective_name,
    )?;

    let used = if summary.convention_file_used {
        "aenv-namespace.toml convention file"
    } else {
        "heuristic layout"
    };
    println!(
        "Imported '{}' from {} ({} file{}, {} director{} captured; via {}).",
        effective_name,
        src_path.display(),
        summary.files_copied,
        if summary.files_copied == 1 { "" } else { "s" },
        summary.directories_copied,
        if summary.directories_copied == 1 {
            "y"
        } else {
            "ies"
        },
        used,
    );
    for p in &summary.user_files_declared {
        println!("  + {p}");
    }
    Ok(())
}

/// True if `source` looks like a git URL. The detector is intentionally
/// conservative: anything else is treated as a local path.
///
/// `file://` URLs are treated as git URLs — `git clone file:///path/to/repo`
/// is the standard way to clone a local repo, and that's how the offline
/// test suite exercises this code path.
pub(crate) fn looks_like_git_url(source: &str) -> bool {
    source.starts_with("https://")
        || source.starts_with("http://")
        || source.starts_with("git://")
        || source.starts_with("git@")
        || source.starts_with("file://")
        || source.ends_with(".git")
}

/// Resolve a local-path source string into an absolute `PathBuf`. The
/// `file://` prefix is reserved for git URLs (see [`looks_like_git_url`]);
/// callers must hand non-git sources as plain filesystem paths.
pub(crate) fn resolve_local_source(source: &str) -> Result<PathBuf> {
    let p = PathBuf::from(source);
    let abs = if p.is_absolute() {
        p
    } else {
        let cwd = std::env::current_dir().map_err(AenvError::Io)?;
        cwd.join(p)
    };
    Ok(abs)
}

/// Derive a default namespace name from a source path: the last non-empty
/// path component. Errors if the path has no usable component (e.g. `/`).
pub(crate) fn default_name_for(source: &Path) -> Result<String> {
    source
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            AenvError::ManifestInvalid(format!(
                "cannot derive a namespace name from source '{}'; pass one explicitly",
                source.display()
            ))
        })
}

/// Clone a git URL into a tempdir, then run the local-path importer against
/// the clone. When `pin` is set, the clone is checked out at that ref after
/// cloning; reuses `aenv_core::skills::git::git_clone`, which already handles
/// SHA-shaped refs vs branch/tag refs correctly.
fn run_git<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    url: &str,
    name: &str,
    pin: Option<&str>,
) -> Result<()> {
    let effective_name = if name.is_empty() {
        default_name_from_url(url)?
    } else {
        name.to_string()
    };

    // Tempdir lives for the duration of the import; the cloned source is
    // discarded once the namespace dir on disk has its own copy.
    let tmp = tempfile::tempdir().map_err(AenvError::Io)?;
    // `git_clone` requires the destination not to exist (git clone creates it).
    let clone_dir = tmp.path().join("clone");
    let resolved = git_clone(url, pin, &clone_dir)?;

    let summary = aenv_core::global_snapshot::import_global(
        fs,
        layout,
        adapters,
        &clone_dir,
        &effective_name,
    )?;

    let used = if summary.convention_file_used {
        "aenv-namespace.toml convention file"
    } else {
        "heuristic layout"
    };
    let pin_disp = pin.map(|p| format!(" @ {p}")).unwrap_or_default();
    println!(
        "Imported '{}' from {}{} (commit {}, {} file{}, {} director{} captured; via {}).",
        effective_name,
        url,
        pin_disp,
        resolved,
        summary.files_copied,
        if summary.files_copied == 1 { "" } else { "s" },
        summary.directories_copied,
        if summary.directories_copied == 1 {
            "y"
        } else {
            "ies"
        },
        used,
    );
    for p in &summary.user_files_declared {
        println!("  + {p}");
    }
    Ok(())
}

/// Derive a default namespace name from a git URL: the last path component
/// with a trailing `.git` stripped. Errors if the URL has no usable name
/// (e.g. `https://example.com/`).
pub(crate) fn default_name_from_url(url: &str) -> Result<String> {
    // Trim any `?query` or `#fragment` tail; git URLs don't generally use
    // them but be defensive.
    let main = url.split(['?', '#']).next().unwrap_or(url);
    // Strip a trailing slash to make rsplit pick the last non-empty segment.
    let trimmed = main.trim_end_matches('/');
    // For `git@host:user/repo.git`, take the part after `:` if no `/` follows
    // the last `:`.
    let last_segment = trimmed.rsplit(['/', ':']).next().unwrap_or(trimmed);
    let cleaned = last_segment.strip_suffix(".git").unwrap_or(last_segment);
    if cleaned.is_empty() {
        return Err(AenvError::ManifestInvalid(format!(
            "cannot derive namespace name from git URL '{url}'; pass one explicitly"
        )));
    }
    Ok(cleaned.to_string())
}
