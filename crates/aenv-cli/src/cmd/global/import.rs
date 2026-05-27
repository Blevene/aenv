//! `aenv global import <source> [<name>]` — turn a directory tree (Task 5)
//! or a git URL (Task 6) into an aenv namespace.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::{Path, PathBuf};

/// Run the import command. Local-path-only for Task 5; git URL support lands
/// in Task 6.
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    source: &str,
    name: &str,
    pin: Option<&str>,
) -> Result<()> {
    if looks_like_git_url(source) {
        return Err(AenvError::ManifestInvalid(format!(
            "git URL imports ('{source}') ship in Task 6; for now, clone manually \
             and import the local path"
        )));
    }
    if pin.is_some() {
        return Err(AenvError::ManifestInvalid(
            "--pin only applies to git URL sources (Task 6)".into(),
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
pub(crate) fn looks_like_git_url(source: &str) -> bool {
    source.starts_with("https://")
        || source.starts_with("http://")
        || source.starts_with("git://")
        || source.starts_with("git@")
        || source.ends_with(".git")
}

/// Resolve a local-path source string into an absolute `PathBuf`. Accepts
/// either `file://` URLs or filesystem paths.
fn resolve_local_source(source: &str) -> Result<PathBuf> {
    let path_str = if let Some(rest) = source.strip_prefix("file://") {
        rest.to_string()
    } else {
        source.to_string()
    };
    let p = PathBuf::from(&path_str);
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
