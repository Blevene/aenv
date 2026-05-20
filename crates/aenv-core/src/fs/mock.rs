//! In-memory `Filesystem` implementation for tests.
//!
//! Stores files and directories in `BTreeMap<PathBuf, Node>` wrapped in a
//! `RefCell` so mutating ops can be called through `&self`. Supports
//! per-path failure injection (writes, stats) so callers can simulate disk
//! full, permission errors, races, etc.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Component, Path, PathBuf};

use super::{FileKind, Filesystem, Metadata};

#[derive(Debug, Clone)]
enum Node {
    File(Vec<u8>),
    Directory,
    Symlink(PathBuf),
}

#[derive(Debug, Default, Clone)]
struct MockState {
    nodes: BTreeMap<PathBuf, Node>,
    /// Paths whose writes should fail with `ErrorKind::Other`.
    write_failures: BTreeSet<PathBuf>,
    /// Paths whose stat-shaped reads (exists, metadata, symlink_metadata,
    /// is_symlink) should fail with `ErrorKind::PermissionDenied`.
    stat_failures: BTreeSet<PathBuf>,
}

/// In-memory filesystem for tests.
///
/// Interior mutability via `RefCell` so callers use `&self` throughout —
/// matching the `Filesystem` trait shape. Single-threaded (not `Sync`).
#[derive(Debug, Default, Clone)]
pub struct MockFilesystem {
    state: RefCell<MockState>,
}

impl MockFilesystem {
    /// Create an empty in-memory filesystem.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cause future writes to `path` to fail with `ErrorKind::Other`.
    pub fn fail_writes_to(&self, path: &Path) {
        self.state
            .borrow_mut()
            .write_failures
            .insert(path.to_path_buf());
    }

    /// Cause future stat-shaped reads on `path` (`exists`, `metadata`,
    /// `symlink_metadata`, `is_symlink`) to fail with `PermissionDenied`.
    ///
    /// This is the mock's hook for testing the `Err` branch of
    /// `Filesystem::exists`, which is the entire reason the return type is
    /// `io::Result<bool>` rather than `bool`.
    pub fn fail_stats_on(&self, path: &Path) {
        self.state
            .borrow_mut()
            .stat_failures
            .insert(path.to_path_buf());
    }
}

impl MockState {
    /// Resolve a path, following symlinks up to 16 levels deep. Returns the
    /// final resolved path and a *clone* of the node at that path. Cloning
    /// is necessary because we can't return references through `RefCell::borrow`.
    fn resolve_owned(&self, path: &Path) -> Option<(PathBuf, Node)> {
        let mut current = path.to_path_buf();
        for _ in 0..16 {
            match self.nodes.get(&current) {
                Some(Node::Symlink(target)) => {
                    current = if target.is_absolute() {
                        target.clone()
                    } else {
                        // POSIX: relative targets resolve against the link's
                        // parent directory.
                        current
                            .parent()
                            .map(|p| p.join(target))
                            .unwrap_or_else(|| target.clone())
                    };
                    current = normalize(&current);
                }
                Some(node) => return Some((current, node.clone())),
                None => return None,
            }
        }
        None
    }

    fn ensure_parents(&mut self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                self.create_dir_all_inner(parent)?;
            }
        }
        Ok(())
    }

    fn create_dir_all_inner(&mut self, path: &Path) -> io::Result<()> {
        // Walk ancestors from root toward `path`, marking each as a directory.
        let mut acc = PathBuf::new();
        for comp in path.components() {
            acc.push(comp);
            match self.nodes.get(&acc) {
                Some(Node::Directory) => {}
                Some(_) => {
                    return Err(io::Error::other(format!(
                        "not a directory: {}",
                        acc.display()
                    )));
                }
                None => {
                    self.nodes.insert(acc.clone(), Node::Directory);
                }
            }
        }
        Ok(())
    }
}

/// Lexically normalize a path: collapse `.` and `..` components without
/// touching the filesystem. Used by symlink resolution so relative targets
/// don't accumulate `..` segments.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    // .. above root — keep it (matches PathBuf semantics).
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

impl Filesystem for MockFilesystem {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        let state = self.state.borrow();
        match state.resolve_owned(path) {
            Some((_, Node::File(bytes))) => Ok(bytes),
            Some((_, Node::Directory)) => Err(io::Error::other("is a directory")),
            Some((_, Node::Symlink(_))) => unreachable!("resolve follows symlinks"),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("not found: {}", path.display()),
            )),
        }
    }

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        let mut state = self.state.borrow_mut();
        if state.write_failures.contains(path) {
            return Err(io::Error::other("injected failure"));
        }
        // Mirror std::fs::write: error if path currently exists as a directory.
        if matches!(state.nodes.get(path), Some(Node::Directory)) {
            return Err(io::Error::other(format!(
                "is a directory: {}",
                path.display()
            )));
        }
        state.ensure_parents(path)?;
        state
            .nodes
            .insert(path.to_path_buf(), Node::File(contents.to_vec()));
        Ok(())
    }

    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()> {
        let mut state = self.state.borrow_mut();
        state.ensure_parents(link)?;
        state
            .nodes
            .insert(link.to_path_buf(), Node::Symlink(target.to_path_buf()));
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut state = self.state.borrow_mut();
        if !state.nodes.contains_key(from) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("not found: {}", from.display()),
            ));
        }
        // Move `from` and every descendant whose path starts with `from`/.
        // Collect first to avoid mutating the map while iterating.
        let movers: Vec<(PathBuf, Node)> = state
            .nodes
            .iter()
            .filter(|(k, _)| k.as_path() == from || k.starts_with(from))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (old_key, _) in &movers {
            state.nodes.remove(old_key);
        }
        // Make sure parent of `to` exists.
        state.ensure_parents(to)?;
        for (old_key, node) in movers {
            let new_key = if old_key.as_path() == from {
                to.to_path_buf()
            } else {
                // Rebase: replace the `from` prefix with `to`.
                let suffix = old_key.strip_prefix(from).expect("starts_with checked");
                to.join(suffix)
            };
            state.nodes.insert(new_key, node);
        }
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        let mut state = self.state.borrow_mut();
        match state.nodes.get(path) {
            Some(Node::Directory) => Err(io::Error::other("is a directory")),
            Some(_) => {
                state.nodes.remove(path);
                Ok(())
            }
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        }
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut state = self.state.borrow_mut();
        match state.nodes.get(path) {
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
            // Mirror std::fs::remove_dir_all: errors when target isn't a directory.
            Some(Node::File(_)) | Some(Node::Symlink(_)) => Err(io::Error::other(format!(
                "not a directory: {}",
                path.display()
            ))),
            Some(Node::Directory) => {
                let prefix = path.to_path_buf();
                let keys: Vec<PathBuf> = state
                    .nodes
                    .keys()
                    .filter(|k| k.as_path() == prefix.as_path() || k.starts_with(&prefix))
                    .cloned()
                    .collect();
                for k in keys {
                    state.nodes.remove(&k);
                }
                Ok(())
            }
        }
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.state.borrow_mut().create_dir_all_inner(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        let state = self.state.borrow();
        if state.stat_failures.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected stat failure",
            ));
        }
        match state.resolve_owned(path) {
            Some((_, Node::File(bytes))) => Ok(Metadata {
                kind: FileKind::File,
                len: bytes.len() as u64,
            }),
            Some((_, Node::Directory)) => Ok(Metadata {
                kind: FileKind::Directory,
                len: 0,
            }),
            Some((_, Node::Symlink(_))) => unreachable!("resolve follows symlinks"),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        }
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<Metadata> {
        let state = self.state.borrow();
        if state.stat_failures.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected stat failure",
            ));
        }
        match state.nodes.get(path) {
            Some(Node::File(bytes)) => Ok(Metadata {
                kind: FileKind::File,
                len: bytes.len() as u64,
            }),
            Some(Node::Directory) => Ok(Metadata {
                kind: FileKind::Directory,
                len: 0,
            }),
            Some(Node::Symlink(_)) => Ok(Metadata {
                kind: FileKind::Symlink,
                len: 0,
            }),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        }
    }

    fn is_symlink(&self, path: &Path) -> io::Result<bool> {
        let state = self.state.borrow();
        if state.stat_failures.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected stat failure",
            ));
        }
        match state.nodes.get(path) {
            Some(Node::Symlink(_)) => Ok(true),
            Some(_) => Ok(false),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        }
    }

    fn read_link(&self, path: &Path) -> io::Result<PathBuf> {
        let state = self.state.borrow();
        match state.nodes.get(path) {
            Some(Node::Symlink(target)) => Ok(target.clone()),
            Some(_) => Err(io::Error::other("not a symlink")),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        }
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        let state = self.state.borrow();
        if state.stat_failures.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected stat failure",
            ));
        }
        Ok(state.resolve_owned(path).is_some())
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let state = self.state.borrow();
        match state.nodes.get(path) {
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
            Some(Node::File(_)) | Some(Node::Symlink(_)) => Err(io::Error::other(format!(
                "not a directory: {}",
                path.display()
            ))),
            Some(Node::Directory) => {
                let mut out = Vec::new();
                for key in state.nodes.keys() {
                    if key.parent() == Some(path) {
                        out.push(key.clone());
                    }
                }
                Ok(out)
            }
        }
    }
}
