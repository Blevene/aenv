//! In-memory `Filesystem` implementation for tests.
//!
//! Stores files and directories in `BTreeMap<PathBuf, Node>`. Supports
//! per-path failure injection so callers can simulate disk full,
//! permission errors, races, etc.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use super::{FileKind, Filesystem, Metadata};

#[derive(Debug, Clone)]
enum Node {
    File(Vec<u8>),
    Directory,
    Symlink(PathBuf),
}

/// In-memory filesystem for tests.
#[derive(Debug, Default, Clone)]
pub struct MockFilesystem {
    nodes: BTreeMap<PathBuf, Node>,
    /// Paths whose writes should fail (for injected error testing).
    write_failures: BTreeSet<PathBuf>,
}

impl MockFilesystem {
    /// Create an empty in-memory filesystem.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cause future writes to `path` to fail with `ErrorKind::Other`.
    pub fn fail_writes_to(&mut self, path: &Path) {
        self.write_failures.insert(path.to_path_buf());
    }

    fn resolve(&self, path: &Path) -> Option<(PathBuf, &Node)> {
        // Follow symlinks up to 16 levels deep to avoid infinite loops.
        let mut current = path.to_path_buf();
        for _ in 0..16 {
            match self.nodes.get(&current) {
                Some(Node::Symlink(target)) => current = target.clone(),
                Some(node) => return Some((current, node)),
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
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("not a directory: {}", acc.display()),
                    ));
                }
                None => {
                    self.nodes.insert(acc.clone(), Node::Directory);
                }
            }
        }
        Ok(())
    }
}

impl Filesystem for MockFilesystem {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        match self.resolve(path) {
            Some((_, Node::File(bytes))) => Ok(bytes.clone()),
            Some((_, Node::Directory)) => Err(io::Error::other("is a directory")),
            Some((_, Node::Symlink(_))) => unreachable!("resolve follows symlinks"),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("not found: {}", path.display()),
            )),
        }
    }

    fn write(&mut self, path: &Path, contents: &[u8]) -> io::Result<()> {
        if self.write_failures.contains(path) {
            return Err(io::Error::other("injected failure"));
        }
        self.ensure_parents(path)?;
        self.nodes
            .insert(path.to_path_buf(), Node::File(contents.to_vec()));
        Ok(())
    }

    fn symlink(&mut self, target: &Path, link: &Path) -> io::Result<()> {
        self.ensure_parents(link)?;
        self.nodes
            .insert(link.to_path_buf(), Node::Symlink(target.to_path_buf()));
        Ok(())
    }

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        let node = self.nodes.remove(from).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("not found: {}", from.display()),
            )
        })?;
        self.ensure_parents(to)?;
        self.nodes.insert(to.to_path_buf(), node);
        Ok(())
    }

    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        match self.nodes.get(path) {
            Some(Node::Directory) => Err(io::Error::other("is a directory")),
            Some(_) => {
                self.nodes.remove(path);
                Ok(())
            }
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        }
    }

    fn remove_dir_all(&mut self, path: &Path) -> io::Result<()> {
        let prefix = path.to_path_buf();
        let keys: Vec<PathBuf> = self
            .nodes
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        if keys.is_empty() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
        }
        for k in keys {
            self.nodes.remove(&k);
        }
        Ok(())
    }

    fn create_dir_all(&mut self, path: &Path) -> io::Result<()> {
        self.create_dir_all_inner(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        match self.resolve(path) {
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
        match self.nodes.get(path) {
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
        match self.nodes.get(path) {
            Some(Node::Symlink(_)) => Ok(true),
            Some(_) => Ok(false),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        }
    }

    fn read_link(&self, path: &Path) -> io::Result<PathBuf> {
        match self.nodes.get(path) {
            Some(Node::Symlink(target)) => Ok(target.clone()),
            Some(_) => Err(io::Error::other("not a symlink")),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        }
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        // The in-memory store never raises permission errors, so this is
        // always Ok. Real and mock both honor the same contract: Ok(false)
        // means "confirmed missing."
        Ok(self.resolve(path).is_some())
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        if !matches!(self.nodes.get(path), Some(Node::Directory)) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "not a directory"));
        }
        let mut out = Vec::new();
        for key in self.nodes.keys() {
            if key.parent() == Some(path) {
                out.push(key.clone());
            }
        }
        Ok(out)
    }
}
