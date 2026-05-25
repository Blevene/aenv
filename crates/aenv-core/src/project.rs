//! `.aenv` pin file IO and project-root resolution.
//!
//! A project pin is a one-name-per-line file at the project root. Phase 1
//! supports a single namespace per project; multi-namespace pins (PRD R-33)
//! arrive with composition in Phase 2.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use std::path::{Path, PathBuf};

/// Filename of the pin file.
pub const PIN_FILENAME: &str = ".aenv";

/// Write `namespace_name` as the pin for `project_root`. Overwrites any
/// existing pin.
pub fn write_pin<F: Filesystem>(fs: &F, project_root: &Path, namespace_name: &str) -> Result<()> {
    let mut content = String::from(namespace_name);
    content.push('\n');
    fs.write(&project_root.join(PIN_FILENAME), content.as_bytes())?;
    Ok(())
}

/// Read the pinned namespace name from `project_root`. Returns
/// `ProjectNotPinned` if no pin file exists; `ManifestInvalid` if the file
/// exists but contains only whitespace.
pub fn read_pin<F: Filesystem>(fs: &F, project_root: &Path) -> Result<String> {
    let path = project_root.join(PIN_FILENAME);
    if !fs.exists(&path)? {
        return Err(AenvError::ProjectNotPinned);
    }
    let bytes = fs.read(&path)?;
    let text = String::from_utf8(bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", path.display())))?;
    // First non-blank, non-comment line wins.
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return Ok(trimmed.to_string());
    }
    Err(AenvError::ManifestInvalid(format!(
        "{}: no namespace name found",
        path.display()
    )))
}

/// Walk up from `start` looking for a `.aenv` pin file. Returns the path
/// containing the nearest-ancestor pin file. Errors `ProjectNotPinned` if
/// no ancestor (or `start` itself) contains one.
///
/// A directory named `.aenv` is NOT a pin — the registry root lives at
/// `$HOME/.aenv` by default, and the walk must step over it instead of
/// treating it as a project root.
pub fn find_project_root<F: Filesystem>(fs: &F, start: &Path) -> Result<PathBuf> {
    let mut current: Option<&Path> = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(PIN_FILENAME);
        if fs.exists(&candidate)? {
            let kind = fs.metadata(&candidate)?.kind;
            if !matches!(kind, crate::fs::FileKind::Directory) {
                return Ok(dir.to_path_buf());
            }
            // Directory at this path is the registry, not a pin. Keep walking.
        }
        current = dir.parent();
    }
    Err(AenvError::ProjectNotPinned)
}
