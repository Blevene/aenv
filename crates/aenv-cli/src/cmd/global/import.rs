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
    shared: bool,
) -> Result<()> {
    if looks_like_git_url(source) {
        return run_git(fs, layout, adapters, source, name, pin, shared);
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
        shared,
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
    // Accept an optional `git+` prefix for parity with `aenv skill import`,
    // whose sources are written `git+<scheme>://…`. The prefix is stripped
    // before the URL reaches `git clone` (see `strip_git_prefix`).
    let s = strip_git_prefix(source);
    s.starts_with("https://")
        || s.starts_with("http://")
        || s.starts_with("git://")
        || s.starts_with("git@")
        || s.starts_with("file://")
        || s.ends_with(".git")
}

/// Strip an optional leading `git+` prefix from a git source string. `git+` is
/// the form `aenv skill import` accepts; `git`/`git clone` itself doesn't
/// understand it, so it must be removed before the URL is cloned or named.
pub(crate) fn strip_git_prefix(source: &str) -> &str {
    source.strip_prefix("git+").unwrap_or(source)
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
    shared: bool,
) -> Result<()> {
    // Normalize away any `git+` prefix — git itself doesn't understand it.
    let url = strip_git_prefix(url);
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
    // Whole-repo import: full clone (no sub_path sparse filter).
    let resolved = git_clone(url, pin, &clone_dir, None)?;

    let summary = aenv_core::global_snapshot::import_global(
        fs,
        layout,
        adapters,
        &clone_dir,
        &effective_name,
        shared,
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
    // Tolerate a `git+` prefix here too, so name derivation works whether or
    // not the caller already stripped it.
    let url = strip_git_prefix(url);
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

#[cfg(test)]
mod tests {
    use super::{default_name_from_url, looks_like_git_url, strip_git_prefix};

    #[test]
    fn detects_bare_and_git_prefixed_urls() {
        // Bare schemes (the forms that already worked).
        assert!(looks_like_git_url("https://github.com/affaan-m/ECC"));
        assert!(looks_like_git_url("git@github.com:affaan-m/ECC.git"));
        assert!(looks_like_git_url("file:///tmp/repo"));
        assert!(looks_like_git_url("https://example.com/x.git"));
        // The `git+` prefix `aenv skill import` uses — the reconciled case.
        assert!(looks_like_git_url("git+https://github.com/affaan-m/ECC"));
        assert!(looks_like_git_url("git+ssh://git@host/repo.git"));
    }

    #[test]
    fn rejects_non_urls() {
        // A plain namespace name must NOT be mistaken for a git URL.
        assert!(!looks_like_git_url("ECC"));
        assert!(!looks_like_git_url("my-profile"));
        // `git+` alone over a non-URL stays a non-URL (conservative).
        assert!(!looks_like_git_url("git+nonsense"));
    }

    #[test]
    fn strip_git_prefix_is_idempotent_on_bare_urls() {
        assert_eq!(
            strip_git_prefix("git+https://github.com/affaan-m/ECC"),
            "https://github.com/affaan-m/ECC"
        );
        assert_eq!(
            strip_git_prefix("https://github.com/affaan-m/ECC"),
            "https://github.com/affaan-m/ECC"
        );
    }

    #[test]
    fn derives_name_regardless_of_git_prefix() {
        assert_eq!(
            default_name_from_url("git+https://github.com/affaan-m/ECC").unwrap(),
            "ECC"
        );
        assert_eq!(
            default_name_from_url("https://github.com/affaan-m/ECC.git").unwrap(),
            "ECC"
        );
    }
}
