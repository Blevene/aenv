//! Filesystem abstraction for `aenv-core`.
//!
//! All disk I/O flows through the `Filesystem` trait. Production code uses
//! [`RealFilesystem`]; tests use the in-memory `MockFilesystem` (see this
//! module's siblings). Keep the trait surface narrow — mocking `std::fs`
//! wholesale is a tar pit; mocking the ~dozen operations `aenv` actually
//! performs is tractable.

use std::io;
use std::path::{Path, PathBuf};

/// What kind of entry a path refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Regular file.
    File,
    /// Directory.
    Directory,
    /// Symbolic link. Note: `Filesystem::metadata` follows symlinks; callers
    /// who want to detect a symlink itself should use `Filesystem::is_symlink`.
    Symlink,
}

/// Minimal metadata about a filesystem entry.
///
/// `aenv` doesn't need timestamps or permissions for any of its current
/// operations; both are deliberately omitted to keep the abstraction small
/// and the mock simple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// Kind of entry (file, directory, symlink).
    pub kind: FileKind,
    /// Length in bytes (0 for directories and symlinks).
    pub len: u64,
}

/// All filesystem operations `aenv` performs.
///
/// All methods take `&self`. Mutating operations on the mock are routed
/// through interior mutability (`RefCell` around the in-memory state) so
/// callers never have to thread `&mut Filesystem` references. `RealFilesystem`
/// is a ZST that ignores `&self` entirely.
///
/// The trait is intentionally NOT `Send + Sync`. Production use is
/// single-threaded; the mock uses `RefCell` for cheap interior mutability.
/// If concurrent use ever becomes needed (e.g. parallel hashing via rayon
/// per engineering §12), swap the mock's `RefCell` for `Mutex` and add a
/// `Send + Sync` bound.
pub trait Filesystem {
    /// Read the entire contents of `path`. Follows symlinks.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;

    /// Write `contents` to `path`, creating or truncating.
    ///
    /// **Contract:** This method shall create any missing parent directories
    /// before writing. All implementations must honor this — Phase 1's
    /// materialization code depends on being able to write to deep paths
    /// without an explicit `create_dir_all` at each call site.
    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;

    /// Create a symlink at `link` pointing to `target`.
    ///
    /// Argument order matches `std::os::unix::fs::symlink`: the *target*
    /// (what the link points at) comes first, the *link* (the new path on
    /// disk) comes second. Remember it as "from data to handle" — same as
    /// `write(path, contents)`, but with target/link in place of path/contents.
    ///
    /// `target` may be absolute or relative; `link` must be absolute.
    /// Relative targets are resolved against `link`'s parent directory at
    /// read time (matching POSIX semantics).
    ///
    /// ```ignore
    /// // Symlink `/project/.claude/skills/foo` -> `/home/user/.aenv/.../foo`
    /// fs.symlink(&namespace_path, &project_path)?;
    /// ```
    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()>;

    /// Atomically rename `from` to `to`. Both must be on the same filesystem
    /// for true atomicity (engineering §7 — the atomicity probe is built on
    /// top of this).
    ///
    /// **Contract:** Renaming a directory moves all of its descendants with
    /// it. Implementations that store paths flat (e.g. the mock's
    /// `BTreeMap`) must rebase every descendant key.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Remove a single file (not a directory). Fails if the path is a directory.
    fn remove_file(&self, path: &Path) -> io::Result<()>;

    /// Recursively remove a directory and all its contents.
    ///
    /// **Contract:** Fails with `NotADirectory` (or platform-equivalent) if
    /// `path` exists but is not a directory.
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Create `path` and all missing parent directories. Idempotent.
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Fetch metadata, following symlinks.
    fn metadata(&self, path: &Path) -> io::Result<Metadata>;

    /// Fetch metadata for `path` itself, without following symlinks.
    ///
    /// Use this when you need to distinguish a symlink from its target —
    /// for example, Phase 1's activation logic checks whether an existing
    /// project path is already an aenv-managed symlink (no-op) vs. a regular
    /// file (must back up). Combining `metadata` + `is_symlink` for the same
    /// question opens a TOCTOU race window; this single call closes it.
    fn symlink_metadata(&self, path: &Path) -> io::Result<Metadata>;

    /// Whether `path` is itself a symlink (not following).
    fn is_symlink(&self, path: &Path) -> io::Result<bool>;

    /// Read the immediate target of a symlink (does not resolve recursively).
    fn read_link(&self, path: &Path) -> io::Result<PathBuf>;

    /// Whether anything exists at `path` (follows symlinks).
    ///
    /// Returns `Err` if the path cannot be stat'd (e.g. permission denied on
    /// an intermediate directory). Distinguishing "missing" from "can't
    /// tell" matters for Phase 1's backup logic: an `Ok(false)` here must
    /// mean "we confirmed it's not there," not "we couldn't check." This is
    /// the same trap `std::path::Path::exists` walked into; we don't repeat it.
    fn exists(&self, path: &Path) -> io::Result<bool>;

    /// List the immediate children of a directory. Order is not guaranteed.
    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;
}

/// Production `Filesystem` impl backed by `std::fs`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFilesystem;

impl Filesystem for RealFilesystem {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    }

    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link)
        }
        #[cfg(windows)]
        {
            // Windows symlink semantics differ for files vs. directories.
            // Phase 7 adds the copy-mode fallback for cases where symlink
            // creation is unprivileged; for now we use `symlink_file` and
            // surface the error to the caller if it fails.
            std::os::windows::fs::symlink_file(target, link)
        }
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        let m = std::fs::metadata(path)?;
        // `metadata` follows symlinks, so we never see Symlink here.
        let kind = if m.is_file() {
            FileKind::File
        } else if m.is_dir() {
            FileKind::Directory
        } else {
            // Unreachable on supported platforms (block/char devices, sockets,
            // FIFOs are outside aenv's universe), but classify as File for the
            // common stat-result shape rather than panicking.
            FileKind::File
        };
        let len = if matches!(kind, FileKind::File) {
            m.len()
        } else {
            0
        };
        Ok(Metadata { kind, len })
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<Metadata> {
        let m = std::fs::symlink_metadata(path)?;
        let ft = m.file_type();
        let kind = if ft.is_symlink() {
            FileKind::Symlink
        } else if ft.is_dir() {
            FileKind::Directory
        } else {
            FileKind::File
        };
        let len = if matches!(kind, FileKind::File) {
            m.len()
        } else {
            0
        };
        Ok(Metadata { kind, len })
    }

    fn is_symlink(&self, path: &Path) -> io::Result<bool> {
        let m = std::fs::symlink_metadata(path)?;
        Ok(m.file_type().is_symlink())
    }

    fn read_link(&self, path: &Path) -> io::Result<PathBuf> {
        std::fs::read_link(path)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        match std::fs::metadata(path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path)? {
            out.push(entry?.path());
        }
        Ok(out)
    }
}

mod mock;
pub use mock::MockFilesystem;
