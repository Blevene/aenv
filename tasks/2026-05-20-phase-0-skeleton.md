# Phase 0 — Project Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the Rust workspace, foundational types (`AenvError`, `Filesystem`), and CI such that every subsequent phase has a clean substrate to build on.

**Architecture:** Cargo workspace with two crates — `aenv-core` (library: all logic, all types) and `aenv-cli` (binary: thin CLI shell that calls into core). All paths below the CLI layer are absolute; the library never reads `current_dir()` or environment variables. `AenvError` is a `thiserror`-derived enum where every variant maps to a documented exit code. `Filesystem` is a trait isolating all I/O; `RealFilesystem` is the production impl, `MockFilesystem` an in-memory implementation for tests.

**Tech Stack:** Rust stable (edition 2021), `clap` v4 (derive), `thiserror`, `std` only for the runtime fs. Dev: `tempfile`, `insta`, `proptest`. GitHub Actions for CI.

**Plan structure:** 8 tasks, each ~5–15 minutes. Tasks 4–6 follow strict TDD. The whole phase should land in a half-day if running smoothly. Phase 0 ends with a tagged-but-not-released `phase-0-complete` annotation on `main` so the next phase has a clear starting commit.

**Repository state at start:** Working tree clean. `main` at `775296c` (the roadmap commit). `pm_docs/` and `tasks/` already tracked.

---

## Post-implementation deltas (Phase 0 + Phase 0.5)

This plan was executed; the resulting commits are `d35e717` → `3650c4a` (Phase 0, tagged `phase-0-complete`), with follow-up cleanup landing in `c5f041a` → `b2f8b31` (Phase 0.5, post-review). The plan body below is the *original intent* — useful as a historical record but no longer the source of truth for what the code looks like. Where the implementation diverged from the plan, the divergence is recorded here.

**Mechanical formatter/linter nudges (applied during execution):**

- `rustfmt` reflowed the literal `assert!(...)` macros in `version.rs` and `error_exit_codes.rs` (the plan's one-line asserts exceeded `max_width = 100`).
- `rustfmt` collapsed the multi-line iterator chain in `real_filesystem.rs::list_dir_returns_immediate_children`.
- `clippy::io_other_error` rewrote `std::io::Error::new(ErrorKind::Other, "boom")` → `std::io::Error::other("boom")`.
- `clippy::bool_assert_comparison` rewrote `assert_eq!(fs.exists(...).unwrap(), false)` → `assert!(!fs.exists(...).unwrap())`.

None changed semantics; the on-disk code is what shipped.

**Phase 0.5 follow-up changes** (driven by an independent code-review pass — see commit `c5f041a` onward; the review's findings are in conversation history, not checked in):

- **`Filesystem` trait flipped from `&mut self` to `&self`** across all mutating methods. `MockFilesystem` now wraps its state in `RefCell` for interior mutability. Callers in Phase 1+ won't thread `&mut`. The trait is intentionally `!Sync`; swap `RefCell` → `Mutex` if concurrency is needed later.
- **Trait grew from 12 to 13 methods.** `symlink_metadata` joined the surface so activation logic can answer "is this an aenv-managed symlink?" without a TOCTOU window. Engineering doc §5 updated to record this.
- **Four mock contract divergences from real fixed** (each had a corresponding test added):
  - `rename` of a directory now rebases every descendant key (was orphaning them).
  - `write` errors when path is currently a directory (was silently overwriting).
  - `remove_dir_all` errors when path is a regular file (was silently removing).
  - `list_dir` distinguishes `NotFound` from "not a directory."
- **Relative symlink targets** now resolve against the link's parent directory (POSIX semantics), with lexical path normalization. New test in `mock_filesystem.rs`.
- **`MockFilesystem::fail_stats_on(path)`** added — makes stat-shaped reads (`exists`, `metadata`, `symlink_metadata`, `is_symlink`) return `PermissionDenied`. This is the hook needed to exercise the `Err` branch of `Filesystem::exists` — without it, the whole point of `exists` returning `io::Result<bool>` was untestable.
- **New "Phase-1-shaped scenario" test** (`phase_1_shaped_scenario_backup_then_restore`) exercises backup → symlink → restore against the mock end-to-end. This is the test that would have caught the mock contract gaps during Phase 0 if it had existed.
- **Two new error-coverage tests:** `io_error_round_trips_via_question_mark_with_exit_one` locks the `#[from] io::Error` conversion path every Phase 1 fs call will use; `all_exit_codes_are_pairwise_distinct` locks PRD R-82 via a `HashSet` equality check.
- **`thiserror` dropped from `aenv-cli/Cargo.toml`** — was declared but unused. Returns when Phase 1's CLI error-formatting layer needs it.
- **CI hardened:** added `cargo doc` (with `RUSTDOCFLAGS=-D warnings` so `missing_docs` actually fails the build), `rustc --version` baked into the cargo cache key, an `x86_64-pc-windows-gnu` cross-compile job to type-check the `#[cfg(windows)]` codepath, an MSRV-1.79 verification job, and a `cargo audit` supply-chain job.
- **`rustfmt.toml`** documents the `imports_granularity = "Crate"` convention as a comment rather than enabling it — the setting is nightly-only and would emit a permanent warning on stable.

**Final state after Phase 0.5:** 46 tests passing across the workspace; `phase-0-complete` tag still points at the Phase 0 commit (`3650c4a`), not the Phase 0.5 head. Phase 0.5 work lives in the commit range `c5f041a..b2f8b31`.

**Phase 0.5 post-push delta:** First CI run on origin revealed that the declared MSRV of 1.79 was too low — `clap_lex 1.1.0` (transitive dep of `clap 4.5`) uses Cargo edition 2024, which requires Cargo 1.85+. MSRV bumped to 1.85 across `Cargo.toml`, `clippy.toml`, `.github/workflows/ci.yml`, and `README.md`. The plan's literal code blocks below still show 1.79; the actual repo is at 1.85.

---

## Prerequisites

The implementing engineer needs Rust stable installed.

- [ ] **Step P1: Install rust via rustup if not present**

```bash
which rustc cargo
```

If both commands print a path, skip to Task 1. Otherwise:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

Expected: both commands print versions (rustc 1.79+ is the declared MSRV; current stable is fine).

- [ ] **Step P2: Add clippy and rustfmt components**

```bash
rustup component add clippy rustfmt
```

Expected: silent success or "component is already installed."

No commit for prerequisites — these affect the local machine, not the repo.

---

## Task 1: Add `.gitignore`

**Files:**
- Create: `/home/angel/Documents/code/aenv/.gitignore`

**Why first:** Stop `target/` and editor cruft from getting staged when the next tasks generate build artifacts.

- [ ] **Step 1.1: Write the .gitignore**

Create `.gitignore` with:

```gitignore
# Rust
/target/
**/*.rs.bk
Cargo.lock.bak

# Editor
.vscode/
.idea/
*.swp
*.swo
.DS_Store

# Project-local aenv state in test/dev projects under this repo
.aenv/state.json
.aenv/backup/
```

Note: `Cargo.lock` is intentionally NOT ignored — this is a binary, and committing the lockfile is the convention for binaries.

- [ ] **Step 1.2: Verify and commit**

```bash
git status
git add .gitignore
git commit -m "$(cat <<'EOF'
Add .gitignore for Rust build artifacts and editor cruft

Cargo.lock intentionally tracked (this is a binary crate).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: `git status` clean afterwards.

---

## Task 2: Create the cargo workspace skeleton

**Files:**
- Create: `/home/angel/Documents/code/aenv/Cargo.toml`
- Create: `/home/angel/Documents/code/aenv/crates/aenv-core/Cargo.toml`
- Create: `/home/angel/Documents/code/aenv/crates/aenv-core/src/lib.rs`
- Create: `/home/angel/Documents/code/aenv/crates/aenv-cli/Cargo.toml`
- Create: `/home/angel/Documents/code/aenv/crates/aenv-cli/src/main.rs`
- Create: `/home/angel/Documents/code/aenv/rustfmt.toml`
- Create: `/home/angel/Documents/code/aenv/clippy.toml`

- [ ] **Step 2.1: Write the workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/aenv-core", "crates/aenv-cli"]

[workspace.package]
version = "0.0.1"
edition = "2021"
# MSRV: workspace deps (clap 4.5, thiserror 1, serde 1, toml 0.8) all
# comfortably support 1.79. Bump if a dep upgrade requires it; do not bump
# without a CI MSRV job to verify the new floor.
rust-version = "1.79"
license = "MIT OR Apache-2.0"
repository = "https://github.com/blevene/aenv"
authors = ["aenv contributors"]

[workspace.dependencies]
# Runtime
clap = { version = "4.5", features = ["derive"] }
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Dev
tempfile = "3.10"
insta = { version = "1.39", features = ["json"] }
proptest = "1.4"

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
```

- [ ] **Step 2.2: Write `crates/aenv-core/Cargo.toml`**

```toml
[package]
name = "aenv-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "Core library for aenv: namespace resolution, materialization, and types."

[dependencies]
thiserror = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2.3: Write `crates/aenv-core/src/lib.rs`**

```rust
//! Core library for `aenv`.
//!
//! This crate holds all logic, types, and traits. The `aenv-cli` binary is
//! a thin shell that translates command-line invocations into calls against
//! this library. No code below this boundary reads `current_dir()` or
//! environment variables — paths are passed in absolute.

#![warn(missing_docs)]
#![warn(clippy::all)]
```

- [ ] **Step 2.4: Write `crates/aenv-cli/Cargo.toml`**

```toml
[package]
name = "aenv-cli"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "Command-line interface for aenv."

[[bin]]
name = "aenv"
path = "src/main.rs"

[dependencies]
aenv-core = { path = "../aenv-core" }
clap = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 2.5: Write a minimal `crates/aenv-cli/src/main.rs`**

For now just enough to compile. Task 3 wires `--version` properly.

```rust
fn main() {
    println!("aenv (skeleton — no commands wired yet)");
}
```

- [ ] **Step 2.6: Write `rustfmt.toml`**

```toml
edition = "2021"
max_width = 100
```

Keep it minimal — we follow stable rustfmt defaults except for the slightly wider line limit.

- [ ] **Step 2.7: Write `clippy.toml`**

```toml
# Minimum supported Rust version — clippy warns about features newer than this.
# Keep in sync with workspace.package.rust-version in the root Cargo.toml.
msrv = "1.79"
```

- [ ] **Step 2.8: Verify workspace builds and lints clean**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: all four commands exit 0. The test command runs zero tests but should not fail.

- [ ] **Step 2.9: Commit**

```bash
git add Cargo.toml Cargo.lock crates/ rustfmt.toml clippy.toml
git commit -m "$(cat <<'EOF'
Add cargo workspace with aenv-core + aenv-cli crates

aenv-core holds all logic; aenv-cli is a thin shell that translates command
invocations into calls against the library. Both crates inherit workspace
metadata. rustfmt and clippy configured with msrv = 1.79 and max_width = 100.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: working tree clean after commit.

---

## Task 3: Wire `aenv --version` via clap (TDD)

**Files:**
- Modify: `crates/aenv-cli/src/main.rs`
- Create: `crates/aenv-cli/tests/version.rs`

**Why TDD here:** This is the first user-facing command and the first integration test. Getting the test harness right matters more than getting the command right.

- [ ] **Step 3.1: Write the failing integration test**

Create `crates/aenv-cli/tests/version.rs`:

```rust
//! Integration tests for `aenv --version` and `aenv -V`.

use std::process::Command;

fn bin() -> std::path::PathBuf {
    // CARGO_BIN_EXE_<name> is set by cargo for integration tests.
    env!("CARGO_BIN_EXE_aenv").into()
}

#[test]
fn version_long_flag_prints_crate_version() {
    let output = Command::new(bin())
        .arg("--version")
        .output()
        .expect("failed to run aenv --version");
    assert!(output.status.success(), "expected success, got {:?}", output);
    let stdout = String::from_utf8(output.stdout).expect("stdout not utf-8");
    let expected = format!("aenv {}", env!("CARGO_PKG_VERSION"));
    assert!(
        stdout.trim() == expected,
        "expected {:?}, got {:?}",
        expected,
        stdout.trim()
    );
}

#[test]
fn version_short_flag_prints_crate_version() {
    let output = Command::new(bin())
        .arg("-V")
        .output()
        .expect("failed to run aenv -V");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout not utf-8");
    let expected = format!("aenv {}", env!("CARGO_PKG_VERSION"));
    assert_eq!(stdout.trim(), expected);
}
```

Note: `env!("CARGO_PKG_VERSION")` resolves to the cli crate's version when this test is compiled inside the cli crate. That's correct here.

- [ ] **Step 3.2: Run the test and watch it fail**

```bash
cargo test --package aenv-cli --test version
```

Expected: test compiles (it's just a binary invocation) and FAILS — the current main prints "aenv (skeleton — no commands wired yet)", not "aenv 0.0.1". Both test cases fail.

- [ ] **Step 3.3: Implement clap with `--version`**

Replace `crates/aenv-cli/src/main.rs` with:

```rust
//! `aenv` command-line entry point.
//!
//! The binary is intentionally thin: parse arguments via clap, dispatch into
//! `aenv-core`, map the result to an exit code. No business logic lives here.

use clap::Parser;

/// Top-level CLI definition.
#[derive(Debug, Parser)]
#[command(
    name = "aenv",
    version,
    about = "Virtual environments for AI coding harness configs",
    long_about = None,
)]
struct Cli {
    // Subcommands land in later phases. For now, `--version` is the only
    // supported invocation; clap derives it from `version` above.
}

fn main() {
    let _cli = Cli::parse();
    // Phase 1 adds subcommand dispatch here. For now, if we reach this point
    // with no subcommand and clap has already handled --version, we exit 0.
}
```

- [ ] **Step 3.4: Run the test and watch it pass**

```bash
cargo test --package aenv-cli --test version
```

Expected: both tests pass.

- [ ] **Step 3.5: Spot-check the help output**

```bash
cargo run --quiet --package aenv-cli -- --help
```

Expected: clap prints a usage block with the description "Virtual environments for AI coding harness configs" and the `-V, --version` flag listed.

- [ ] **Step 3.6: Lint and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-cli/
git commit -m "$(cat <<'EOF'
Wire aenv --version via clap with integration test

Adds clap derive-based Cli struct with the description that will surface in
--help. Integration test in tests/version.rs invokes the built binary and
asserts the version string matches CARGO_PKG_VERSION. Both --version and -V
covered.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: clean working tree.

---

## Task 4: `AenvError` enum (TDD)

**Files:**
- Create: `crates/aenv-core/src/error.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/error_exit_codes.rs`

**Why this matters:** Exit codes are public contract per PRD R-82/R-83. Locking them into a centralized enum *now* prevents Phase 1+ code from inventing ad-hoc codes inline.

- [ ] **Step 4.1: Write the failing test**

Create `crates/aenv-core/tests/error_exit_codes.rs`:

```rust
//! Locks the exit-code contract from PRD R-82.
//!
//! These codes are public; changing them is a major-version break.

use aenv_core::AenvError;

#[test]
fn generic_io_maps_to_exit_one() {
    let err = AenvError::Io(std::io::Error::new(std::io::ErrorKind::Other, "boom"));
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn namespace_not_found_is_ten() {
    let err = AenvError::NamespaceNotFound("missing".to_string());
    assert_eq!(err.exit_code(), 10);
}

#[test]
fn adapter_missing_is_eleven() {
    let err = AenvError::AdapterMissing("nope".to_string());
    assert_eq!(err.exit_code(), 11);
}

#[test]
fn manifest_invalid_is_twelve() {
    let err = AenvError::ManifestInvalid("bad toml".to_string());
    assert_eq!(err.exit_code(), 12);
}

#[test]
fn activation_conflict_is_thirteen() {
    let err = AenvError::ActivationConflict("file exists".to_string());
    assert_eq!(err.exit_code(), 13);
}

#[test]
fn remote_unreachable_is_fourteen() {
    let err = AenvError::RemoteUnreachable("git fetch failed".to_string());
    assert_eq!(err.exit_code(), 14);
}

#[test]
fn extends_cycle_is_fifteen() {
    let err = AenvError::ExtendsCycle("a -> b -> a".to_string());
    assert_eq!(err.exit_code(), 15);
}

#[test]
fn parameter_undefined_is_sixteen() {
    let err = AenvError::ParameterUndefined("foo.bar".to_string());
    assert_eq!(err.exit_code(), 16);
}

#[test]
fn policy_violation_is_seventeen() {
    let err = AenvError::PolicyViolation("oversize".to_string());
    assert_eq!(err.exit_code(), 17);
}

#[test]
fn project_not_pinned_is_twenty() {
    let err = AenvError::ProjectNotPinned;
    assert_eq!(err.exit_code(), 20);
}

#[test]
fn display_includes_namespace_in_not_found_message() {
    // PRD-driven: error messages should use the "namespace" vocabulary in
    // user-visible output (engineering doc §3 rationale).
    let err = AenvError::NamespaceNotFound("foo".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("namespace"), "expected 'namespace' in {:?}", msg);
    assert!(msg.contains("foo"), "expected 'foo' in {:?}", msg);
}
```

- [ ] **Step 4.2: Run the test and watch it fail**

```bash
cargo test --package aenv-core --test error_exit_codes
```

Expected: FAIL with "cannot find type/variant `AenvError`" (the type doesn't exist yet).

- [ ] **Step 4.3: Implement the error enum**

Create `crates/aenv-core/src/error.rs`:

```rust
//! Public error type for `aenv-core`.
//!
//! Every variant maps to a documented exit code (PRD R-82). The CLI layer is
//! the only place that turns `AenvError` into an exit code; library callers
//! match on the variant.

use std::io;
use thiserror::Error;

/// All errors produced by `aenv-core`.
#[derive(Debug, Error)]
pub enum AenvError {
    /// Namespace name does not exist in the registry. Exit 10.
    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),

    /// Manifest names an adapter that is not installed. Exit 11.
    #[error("adapter not installed: {0}")]
    AdapterMissing(String),

    /// Manifest is malformed or contains an invalid value. Exit 12.
    #[error("manifest invalid: {0}")]
    ManifestInvalid(String),

    /// File materialization conflicts (e.g. atomicity probe failed). Exit 13.
    #[error("activation conflict: {0}")]
    ActivationConflict(String),

    /// Remote git operation failed. Exit 14.
    #[error("remote unreachable: {0}")]
    RemoteUnreachable(String),

    /// Cycle detected in `extends` chain. Exit 15.
    #[error("cycle in extends chain: {0}")]
    ExtendsCycle(String),

    /// `aenv get` named a parameter not declared by the resolution chain. Exit 16.
    #[error("parameter '{0}' is undefined in the resolution chain")]
    ParameterUndefined(String),

    /// Policy with `enforce = true` is violated. Exit 17.
    #[error("policy violation: {0}")]
    PolicyViolation(String),

    /// No `.aenv` pin and no `--project` flag. Exit 20.
    #[error("project not pinned")]
    ProjectNotPinned,

    /// I/O error from the underlying filesystem. Exit 1.
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

impl AenvError {
    /// Map this error to the documented exit code from PRD R-82.
    pub fn exit_code(&self) -> i32 {
        match self {
            AenvError::Io(_) => 1,
            AenvError::NamespaceNotFound(_) => 10,
            AenvError::AdapterMissing(_) => 11,
            AenvError::ManifestInvalid(_) => 12,
            AenvError::ActivationConflict(_) => 13,
            AenvError::RemoteUnreachable(_) => 14,
            AenvError::ExtendsCycle(_) => 15,
            AenvError::ParameterUndefined(_) => 16,
            AenvError::PolicyViolation(_) => 17,
            AenvError::ProjectNotPinned => 20,
        }
    }
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, AenvError>;
```

- [ ] **Step 4.4: Re-export from the library**

Modify `crates/aenv-core/src/lib.rs` to add the module and re-export:

```rust
//! Core library for `aenv`.
//!
//! This crate holds all logic, types, and traits. The `aenv-cli` binary is
//! a thin shell that translates command-line invocations into calls against
//! this library. No code below this boundary reads `current_dir()` or
//! environment variables — paths are passed in absolute.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod error;

pub use error::{AenvError, Result};
```

- [ ] **Step 4.5: Run the test and watch it pass**

```bash
cargo test --package aenv-core --test error_exit_codes
```

Expected: all 11 tests pass.

- [ ] **Step 4.6: Lint and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add AenvError enum with exit-code mapping

Every variant maps to a documented exit code (PRD R-82). Locking this in now
prevents Phase 1+ code from inventing ad-hoc codes inline. Integration test
covers each variant's exit code plus the Display message format for
NamespaceNotFound (verifies the public vocabulary uses "namespace", per
engineering doc §3).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: clean working tree.

---

## Task 5: `Filesystem` trait + `RealFilesystem` (TDD)

**Files:**
- Create: `crates/aenv-core/src/fs.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/real_filesystem.rs`

**Why TDD here:** The trait surface is the most consequential decision in Phase 0. Get the methods wrong and every Phase 1 implementation has to fight the abstraction. Writing tests against `RealFilesystem` first forces the surface to be ergonomic for callers.

The trait covers 13 methods: the 10 listed in engineering doc §5, plus `is_symlink` and `list_dir` (Phase 1 state introspection), plus `symlink_metadata` (TOCTOU-free symlink detection during activation). Engineering §5 said "~12 total"; we're one above and call this consciously. Two methods carry contractual nuance worth flagging up-front:

- `write(path, contents)` **must** create missing parent directories. This is part of the trait contract, not an impl detail; future `Filesystem` implementations are bound by it.
- `exists(path)` returns `io::Result<bool>`, not `bool`. `Ok(false)` means "we confirmed it's missing"; `Err` means "we couldn't tell." `std::path::Path::exists` walked into this trap; we decline to inherit it.

- [ ] **Step 5.1: Write the failing test**

Create `crates/aenv-core/tests/real_filesystem.rs`:

```rust
//! Integration tests for `RealFilesystem` against a real `tempfile::tempdir`.
//!
//! These tests pin the `Filesystem` trait surface to operations that Phase 1
//! materialization actually performs.

use aenv_core::fs::{FileKind, Filesystem, RealFilesystem};
use std::path::PathBuf;
use tempfile::tempdir;

fn rfs() -> RealFilesystem {
    RealFilesystem
}

#[test]
fn write_then_read_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("hello.txt");
    let mut fs = rfs();

    fs.write(&path, b"hello world").unwrap();
    let read = fs.read(&path).unwrap();
    assert_eq!(read, b"hello world");
}

#[test]
fn exists_returns_ok_false_for_missing() {
    let dir = tempdir().unwrap();
    let fs = rfs();
    assert_eq!(fs.exists(&dir.path().join("nope")).unwrap(), false);
}

#[test]
fn exists_returns_ok_true_after_write() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("file");
    let mut fs = rfs();
    fs.write(&path, b"x").unwrap();
    assert_eq!(fs.exists(&path).unwrap(), true);
}

#[test]
fn create_dir_all_is_idempotent() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("a/b/c");
    let mut fs = rfs();
    fs.create_dir_all(&nested).unwrap();
    fs.create_dir_all(&nested).unwrap(); // second call is a no-op
    assert!(fs.exists(&nested).unwrap());
}

#[cfg(unix)]
#[test]
fn symlink_then_read_link_roundtrip() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");
    let mut fs = rfs();

    fs.write(&target, b"target contents").unwrap();
    fs.symlink(&target, &link).unwrap();

    assert!(fs.is_symlink(&link).unwrap());
    assert_eq!(fs.read_link(&link).unwrap(), target);

    // Reading through the symlink returns the target contents.
    assert_eq!(fs.read(&link).unwrap(), b"target contents");
}

#[test]
fn rename_moves_file() {
    let dir = tempdir().unwrap();
    let from = dir.path().join("a");
    let to = dir.path().join("b");
    let mut fs = rfs();

    fs.write(&from, b"data").unwrap();
    fs.rename(&from, &to).unwrap();

    assert!(!fs.exists(&from).unwrap());
    assert!(fs.exists(&to).unwrap());
}

#[test]
fn remove_file_deletes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("kill-me");
    let mut fs = rfs();
    fs.write(&path, b"x").unwrap();
    fs.remove_file(&path).unwrap();
    assert!(!fs.exists(&path).unwrap());
}

#[test]
fn remove_dir_all_deletes_tree() {
    let dir = tempdir().unwrap();
    let tree = dir.path().join("a/b/c");
    let mut fs = rfs();
    fs.create_dir_all(&tree).unwrap();
    fs.write(&tree.join("leaf"), b"x").unwrap();

    fs.remove_dir_all(&dir.path().join("a")).unwrap();
    assert!(!fs.exists(&tree).unwrap());
}

#[test]
fn write_auto_creates_parent_directories() {
    // Part of the Filesystem trait contract: write() creates missing
    // parents. Phase 1 materialization depends on this.
    let dir = tempdir().unwrap();
    let deep = dir.path().join("a/b/c/leaf.txt");
    let mut fs = rfs();
    fs.write(&deep, b"hi").unwrap();
    assert_eq!(fs.read(&deep).unwrap(), b"hi");
}

#[cfg(unix)]
#[test]
fn symlink_metadata_reports_symlink_kind_not_target() {
    // metadata() follows symlinks; symlink_metadata() does not. Phase 1
    // activation logic relies on this distinction to detect aenv-managed
    // symlinks without backing up the underlying target file.
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");
    let mut fs = rfs();

    fs.write(&target, b"target contents").unwrap();
    fs.symlink(&target, &link).unwrap();

    assert_eq!(fs.metadata(&link).unwrap().kind, FileKind::File);
    assert_eq!(fs.symlink_metadata(&link).unwrap().kind, FileKind::Symlink);
}

#[test]
fn metadata_reports_kind_and_size() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("f");
    let mut fs = rfs();
    fs.write(&file, b"abcd").unwrap();

    let meta = fs.metadata(&file).unwrap();
    assert_eq!(meta.kind, FileKind::File);
    assert_eq!(meta.len, 4);
}

#[test]
fn metadata_distinguishes_directory_from_file() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("d");
    let mut fs = rfs();
    fs.create_dir_all(&nested).unwrap();

    let meta = fs.metadata(&nested).unwrap();
    assert_eq!(meta.kind, FileKind::Directory);
}

#[test]
fn list_dir_returns_immediate_children() {
    let dir = tempdir().unwrap();
    let mut fs = rfs();
    fs.write(&dir.path().join("a"), b"x").unwrap();
    fs.write(&dir.path().join("b"), b"y").unwrap();
    fs.create_dir_all(&dir.path().join("c")).unwrap();

    let mut entries: Vec<PathBuf> = fs.list_dir(dir.path()).unwrap();
    entries.sort();
    let expected: Vec<PathBuf> = ["a", "b", "c"]
        .iter()
        .map(|n| dir.path().join(n))
        .collect();
    assert_eq!(entries, expected);
}
```

- [ ] **Step 5.2: Run the test and watch it fail**

```bash
cargo test --package aenv-core --test real_filesystem
```

Expected: FAIL with "unresolved import" — `aenv_core::fs` doesn't exist yet.

- [ ] **Step 5.3: Implement the trait, `RealFilesystem`, and `Metadata`**

Create `crates/aenv-core/src/fs.rs`:

```rust
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
/// Methods take `&mut self` where they mutate the filesystem so the mock can
/// hold its in-memory state behind a single borrow; `RealFilesystem` is a
/// zero-sized type so `&mut self` is free.
pub trait Filesystem {
    /// Read the entire contents of `path`. Follows symlinks.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;

    /// Write `contents` to `path`, creating or truncating.
    ///
    /// **Contract:** This method shall create any missing parent directories
    /// before writing. All implementations must honor this — Phase 1's
    /// materialization code depends on being able to write to deep paths
    /// without an explicit `create_dir_all` at each call site.
    fn write(&mut self, path: &Path, contents: &[u8]) -> io::Result<()>;

    /// Create a symlink at `link` pointing to `target`.
    ///
    /// `target` may be absolute or relative; `link` must be absolute.
    fn symlink(&mut self, target: &Path, link: &Path) -> io::Result<()>;

    /// Atomically rename `from` to `to`. Both must be on the same filesystem
    /// for true atomicity (engineering §7 — the atomicity probe is built on
    /// top of this).
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;

    /// Remove a single file (not a directory). Fails if the path is a directory.
    fn remove_file(&mut self, path: &Path) -> io::Result<()>;

    /// Recursively remove a directory and all its contents.
    fn remove_dir_all(&mut self, path: &Path) -> io::Result<()>;

    /// Create `path` and all missing parent directories. Idempotent.
    fn create_dir_all(&mut self, path: &Path) -> io::Result<()>;

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

    fn write(&mut self, path: &Path, contents: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    }

    fn symlink(&mut self, target: &Path, link: &Path) -> io::Result<()> {
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

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn remove_dir_all(&mut self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn create_dir_all(&mut self, path: &Path) -> io::Result<()> {
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
```

- [ ] **Step 5.4: Re-export from the library**

Modify `crates/aenv-core/src/lib.rs` to register the module:

```rust
//! Core library for `aenv`.
//!
//! This crate holds all logic, types, and traits. The `aenv-cli` binary is
//! a thin shell that translates command-line invocations into calls against
//! this library. No code below this boundary reads `current_dir()` or
//! environment variables — paths are passed in absolute.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod error;
pub mod fs;

pub use error::{AenvError, Result};
```

- [ ] **Step 5.5: Run the test and watch it pass**

```bash
cargo test --package aenv-core --test real_filesystem
```

Expected: all 11 tests pass.

- [ ] **Step 5.6: Lint and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add Filesystem trait and RealFilesystem with std-backed impl

Trait covers 13 methods: 10 listed in engineering doc §5 plus is_symlink,
list_dir, and symlink_metadata. symlink_metadata is added so activation
logic can answer "is this an aenv-managed symlink?" in one call without a
TOCTOU race window between metadata() and is_symlink(). Metadata struct is
intentionally minimal (kind + len) because no current aenv operation needs
mtime or permissions; widening it would complicate the mock.

Two contract points worth flagging:
- write() must create missing parent directories. Phase 1 materialization
  depends on this; making it part of the trait contract (not an impl
  detail) keeps the mock and any future Filesystem impl honest.
- exists() returns io::Result<bool>. Ok(false) means "confirmed missing";
  Err means "couldn't stat." This is the std::path::Path::exists wart we
  decline to inherit.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: clean working tree.

---

## Task 6: `MockFilesystem` (TDD)

**Files:**
- Create: `crates/aenv-core/src/fs/mock.rs`
- Modify: `crates/aenv-core/src/fs.rs` (turn into a directory module)
- Modify: `crates/aenv-core/src/lib.rs` (re-export `MockFilesystem`)
- Create: `crates/aenv-core/tests/mock_filesystem.rs`

**Why this matters:** Every Phase 1 unit test that wants to inject failure conditions (disk full, permission denied, file appearing between check and write) needs `MockFilesystem`. Building it now means Phase 1 doesn't get blocked the moment we want to test rollback.

**Refactor note:** Promote `fs.rs` to `fs/mod.rs` so `mock.rs` is a sibling.

- [ ] **Step 6.1: Refactor `fs.rs` to a directory module**

```bash
mkdir -p crates/aenv-core/src/fs
git mv crates/aenv-core/src/fs.rs crates/aenv-core/src/fs/mod.rs
cargo build --workspace
```

Expected: build succeeds (paths via `pub mod fs;` still resolve because `fs/mod.rs` is equivalent to `fs.rs`).

- [ ] **Step 6.2: Write the failing test**

Create `crates/aenv-core/tests/mock_filesystem.rs`:

```rust
//! Tests for `MockFilesystem` — verifies it honors the same `Filesystem`
//! contract as `RealFilesystem` for the operations Phase 1 will rely on.

use aenv_core::fs::{FileKind, Filesystem, MockFilesystem};
use std::path::PathBuf;

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[test]
fn empty_mock_has_nothing() {
    let fs = MockFilesystem::new();
    assert_eq!(fs.exists(&p("/anything")).unwrap(), false);
}

#[test]
fn write_then_read_roundtrip() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/a/b/c.txt"), b"hello").unwrap();
    assert_eq!(fs.read(&p("/a/b/c.txt")).unwrap(), b"hello");
}

#[test]
fn write_auto_creates_parent_dirs() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/a/b/c.txt"), b"x").unwrap();
    let meta = fs.metadata(&p("/a/b")).unwrap();
    assert_eq!(meta.kind, FileKind::Directory);
}

#[test]
fn rename_moves_file() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/from"), b"data").unwrap();
    fs.rename(&p("/from"), &p("/to")).unwrap();
    assert!(!fs.exists(&p("/from")).unwrap());
    assert_eq!(fs.read(&p("/to")).unwrap(), b"data");
}

#[test]
fn remove_file_deletes() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/x"), b"x").unwrap();
    fs.remove_file(&p("/x")).unwrap();
    assert!(!fs.exists(&p("/x")).unwrap());
}

#[test]
fn remove_dir_all_deletes_tree() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/a/b/c"), b"x").unwrap();
    fs.write(&p("/a/d"), b"y").unwrap();
    fs.remove_dir_all(&p("/a")).unwrap();
    assert!(!fs.exists(&p("/a")).unwrap());
    assert!(!fs.exists(&p("/a/b/c")).unwrap());
}

#[test]
fn symlink_metadata_reports_symlink_kind_not_target() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/target"), b"t").unwrap();
    fs.symlink(&p("/target"), &p("/link")).unwrap();
    assert_eq!(fs.metadata(&p("/link")).unwrap().kind, FileKind::File);
    assert_eq!(fs.symlink_metadata(&p("/link")).unwrap().kind, FileKind::Symlink);
}

#[test]
fn symlink_records_target() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/target"), b"t").unwrap();
    fs.symlink(&p("/target"), &p("/link")).unwrap();
    assert!(fs.is_symlink(&p("/link")).unwrap());
    assert_eq!(fs.read_link(&p("/link")).unwrap(), p("/target"));
}

#[test]
fn read_follows_symlink() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/target"), b"t").unwrap();
    fs.symlink(&p("/target"), &p("/link")).unwrap();
    assert_eq!(fs.read(&p("/link")).unwrap(), b"t");
}

#[test]
fn list_dir_returns_immediate_children() {
    let mut fs = MockFilesystem::new();
    fs.write(&p("/d/a"), b"x").unwrap();
    fs.write(&p("/d/b"), b"y").unwrap();
    fs.create_dir_all(&p("/d/sub")).unwrap();

    let mut entries: Vec<PathBuf> = fs.list_dir(&p("/d")).unwrap();
    entries.sort();
    assert_eq!(entries, vec![p("/d/a"), p("/d/b"), p("/d/sub")]);
}

#[test]
fn injected_failures_propagate() {
    // The mock supports per-path failure injection so Phase 1 can test
    // mid-activation IO errors.
    let mut fs = MockFilesystem::new();
    fs.fail_writes_to(&p("/cursed"));
    let result = fs.write(&p("/cursed"), b"x");
    assert!(result.is_err(), "expected injected failure");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Other);
}
```

- [ ] **Step 6.3: Run the test and watch it fail**

```bash
cargo test --package aenv-core --test mock_filesystem
```

Expected: FAIL with "unresolved import" — `MockFilesystem` doesn't exist.

- [ ] **Step 6.4: Implement `MockFilesystem`**

Create `crates/aenv-core/src/fs/mock.rs`:

```rust
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
            Some((_, Node::Directory)) => Err(io::Error::new(
                io::ErrorKind::Other,
                "is a directory",
            )),
            Some((_, Node::Symlink(_))) => unreachable!("resolve follows symlinks"),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("not found: {}", path.display()),
            )),
        }
    }

    fn write(&mut self, path: &Path, contents: &[u8]) -> io::Result<()> {
        if self.write_failures.contains(path) {
            return Err(io::Error::new(io::ErrorKind::Other, "injected failure"));
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
            io::Error::new(io::ErrorKind::NotFound, format!("not found: {}", from.display()))
        })?;
        self.ensure_parents(to)?;
        self.nodes.insert(to.to_path_buf(), node);
        Ok(())
    }

    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        match self.nodes.get(path) {
            Some(Node::Directory) => Err(io::Error::new(
                io::ErrorKind::Other,
                "is a directory",
            )),
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
            Some(_) => Err(io::Error::new(io::ErrorKind::Other, "not a symlink")),
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
```

- [ ] **Step 6.5: Wire it into `fs/mod.rs`**

Modify `crates/aenv-core/src/fs/mod.rs` to add at the end (after the existing `RealFilesystem` impl):

```rust
mod mock;
pub use mock::MockFilesystem;
```

- [ ] **Step 6.6: Run the test and watch it pass**

```bash
cargo test --package aenv-core --test mock_filesystem
```

Expected: all 10 tests pass.

- [ ] **Step 6.7: Confirm everything still compiles together**

```bash
cargo test --workspace
```

Expected: all tests pass (the existing `error_exit_codes`, `real_filesystem`, `version`, and new `mock_filesystem` all green).

- [ ] **Step 6.8: Lint and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add MockFilesystem for in-memory testing

Implements the full Filesystem trait against a BTreeMap-backed in-memory
store. Symlinks are followed transparently by read() and metadata() (with a
16-level depth limit); is_symlink() and read_link() let callers inspect the
link itself. Per-path write failure injection (fail_writes_to) gives Phase 1
a hook for testing rollback on mid-activation IO errors.

Promoted fs.rs to fs/mod.rs to host the new mock.rs sibling.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: clean working tree.

---

## Task 7: CI workflow (Linux + macOS)

**Files:**
- Create: `.github/workflows/ci.yml`

**Why now:** Up through Task 6 the only thing keeping the build clean is local discipline. From here on, every PR should be gated on CI. Adding it before Phase 1 starts means Phase 1's commits land on a green baseline.

- [ ] **Step 7.1: Write the workflow**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4

      # GitHub-hosted runners (ubuntu-latest, macos-latest) ship with rustup
      # and a current stable toolchain pre-installed. Using the bundled one
      # avoids depending on an unpinned third-party action; the trade-off is
      # we run against whatever stable GitHub ships, which moves over time.
      # We rely on Cargo.toml's rust-version (MSRV) for compatibility.
      - name: Setup Rust toolchain
        run: |
          rustup default stable
          rustup component add rustfmt clippy
          rustc --version
          cargo --version

      - name: Cache cargo registry and target
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: Format check
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: Build
        run: cargo build --workspace --verbose

      - name: Test
        run: cargo test --workspace --verbose
```

Windows is intentionally absent — Phase 7 adds it once the symlink fallback exists.

- [ ] **Step 7.2: Commit the workflow**

```bash
mkdir -p .github/workflows
# (file already created above)
git add .github/workflows/ci.yml
git commit -m "$(cat <<'EOF'
Add GitHub Actions CI for Linux + macOS

Runs fmt --check, clippy with -D warnings, build, and test on every PR and
push to main. Windows runner intentionally absent until Phase 7 adds the
symlink copy-mode fallback. Cargo registry and target are cached keyed on
Cargo.lock to keep CI fast on subsequent runs.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: clean working tree. CI will run on the next push.

---

## Task 8: README pointing at the spec

**Files:**
- Create: `README.md`

**Why:** Someone landing on the repo root needs to know what they're looking at and where to start.

- [ ] **Step 8.1: Write the README**

Create `README.md`:

```markdown
# aenv — Virtual environments for AI coding harness configs

`aenv` is a Rust CLI for managing named, composable, version-controlled bundles of AI-coding-agent configuration (`CLAUDE.md`, `.cursorrules`, `.mcp.json`, skills, agents, slash commands, MCP entries). Think Python's `venv`, but for the rules and configurations that shape how AI coding agents behave.

> **Status:** Early development. The CLI does not yet do anything useful — Phase 0 (project skeleton) is the most recent milestone. See [`tasks/todo.md`](./tasks/todo.md) for the full roadmap.

## Reading order

- **[`pm_docs/aenv-prd.md`](./pm_docs/aenv-prd.md)** — Product requirements in EARS format. The public contract.
- **[`pm_docs/aenv-functional-spec.md`](./pm_docs/aenv-functional-spec.md)** — How users interact with `aenv`. Three example harnesses, twelve user journeys.
- **[`pm_docs/aenv-engineering.md`](./pm_docs/aenv-engineering.md)** — Internal implementation decisions (Rust, crate selection, error/exit-code strategy, `Filesystem` trait, namespace identity model).
- **[`tasks/todo.md`](./tasks/todo.md)** — Implementation roadmap with phase mapping back to PRD requirements.

## Building

```bash
cargo build --workspace
cargo test --workspace
```

Requires Rust stable 1.79 or later.

## License

Dual-licensed under MIT or Apache 2.0.
```

- [ ] **Step 8.2: Commit the README**

```bash
git add README.md
git commit -m "$(cat <<'EOF'
Add README pointing at pm_docs and the roadmap

Marks the repo as early-development and routes readers to the spec bundle
and the phase-level roadmap. Build instructions are deliberately minimal —
the CLI doesn't yet do anything useful.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: clean working tree.

---

## Phase completion verification

Run the full phase-0 acceptance suite:

- [ ] **Step V1: Confirm build and tests are green**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

All four must exit 0.

- [ ] **Step V2: Confirm the binary runs**

```bash
cargo run --quiet --package aenv-cli -- --version
```

Expected output: `aenv 0.0.1`.

- [ ] **Step V3: Confirm every test binary reports `ok` and nothing reports `FAILED`**

```bash
cargo test --workspace 2>&1 | tee /tmp/aenv-phase0-test.log
grep -E "^test result:" /tmp/aenv-phase0-test.log
# Should print one `test result: ok.` line per test binary and nothing else.
! grep -E "test result: FAILED" /tmp/aenv-phase0-test.log
# Sanity check: at least 25 tests should have run across the workspace.
total=$(grep -E "^test result: ok\." /tmp/aenv-phase0-test.log \
  | awk '{ sum += $4 } END { print sum }')
echo "total tests: $total"
test "$total" -ge 25
```

Expected: every line emitted by the first `grep` starts with `test result: ok.`; the second `grep` finds nothing (exits non-zero, which the leading `!` inverts to success); the final `test` exits 0.

Why not lock exact counts: Phase 1+ will add tests; brittle count assertions would force every later phase to update this step. Asserting "all green" + "at least N" gives the same signal without the maintenance tax.

- [ ] **Step V4: Confirm git history is clean**

```bash
git log --oneline -10
```

Expected: 8 new commits since the roadmap commit (`775296c`), in this order:
1. `.gitignore`
2. cargo workspace skeleton
3. `aenv --version`
4. AenvError enum
5. Filesystem trait + RealFilesystem
6. MockFilesystem
7. CI workflow
8. README

- [ ] **Step V5: Tag phase completion**

```bash
git tag -a phase-0-complete -m "Phase 0 — project skeleton complete"
git tag -l
```

Tags should now list `phase-0-complete`. Tag stays local until we push.

## Phase 0 → Phase 1 handoff

What Phase 1 inherits:
- A workspace that compiles, lints clean, and runs tests on CI.
- `AenvError` enum with every documented exit code locked in.
- `Filesystem` trait with `RealFilesystem` and `MockFilesystem`.
- `aenv --version` proves the CLI plumbing works end-to-end.

What Phase 1 picks up (cross-reference `tasks/todo.md`):
- Manifest parsing
- Adapter file parsing + the first built-in (`claude-code`)
- `aenv create / list / delete / use / activate / deactivate / status / restore`
- State file (`.aenv/state.json`)
- Rename atomicity probe
- Rollback-on-failure
- `--project <path>` flag

Open at the boundary: confirm whether `Filesystem::metadata` should grow `mtime` for the byte-identical detection in R-46. Current call: NO — Phase 1 will compare contents byte-wise rather than relying on mtime, which is the right call for cross-platform determinism. Note this here so it doesn't get re-litigated.

---

## Self-review checklist

Run by the planner before handoff:

**1. Spec coverage:** Phase 0 has no PRD requirements directly assigned. It sets up R-82 (exit codes) and engineering §5 (Filesystem trait) substrate. ✓

**2. Placeholder scan:** No "TBD", "implement later", "appropriate", or "similar to" references in tasks. Every code block is complete. ✓

**3. Type consistency:**
- `AenvError` variants are referenced by name in Task 4 tests and definition. Match. ✓
- `Filesystem` trait method signatures in Task 5 (`mod.rs`) match calls in Task 5 and Task 6 tests. ✓
- `Metadata` struct: `kind: FileKind` + `len: u64` consistent across definition (Task 5), `RealFilesystem` impl, and `MockFilesystem` impl (Task 6). ✓
- `FileKind` variants: `File`, `Directory`, `Symlink` consistent across uses. ✓
- `MockFilesystem::fail_writes_to` signature in Task 6 test matches definition. ✓

All checks pass. Plan ready to execute.
