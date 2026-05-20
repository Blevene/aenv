# Phase 1 — Single-Namespace Happy Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A user can create a namespace, pin a project to it, activate it (files materialize as symlinks with displaced originals backed up), deactivate (restore backups, remove symlinks), and restore from backup independently. One adapter only (claude-code). No composition, no parameters, no policies.

**Architecture:** All logic lives in `aenv-core`; `aenv-cli` is a thin subcommand dispatcher. The library is filesystem-injected via the `Filesystem` trait built in Phase 0 — production code uses `RealFilesystem`, tests use `MockFilesystem` with failure injection (`fail_writes_to`, `fail_stats_on`) where rollback paths are exercised. End-to-end integration tests run against real `tempfile::tempdir()`. State file `.aenv/state.json` carries `schema_version: 1` for forward-compat. Rename atomicity is probed at activation start; cross-filesystem boundaries abort with `ActivationConflict` (exit 13).

**Tech Stack:** Rust 1.85+ stable. New library deps: `serde` (already in workspace), `toml` (already in workspace), `serde_json` (already in workspace). Existing trait surface from Phase 0 is sufficient — no new `Filesystem` methods needed.

**Plan structure:** 16 tasks. Tasks 1–4 build pure parsing/types (unit-testable, no fs). Tasks 5–11 build the activation primitives against `MockFilesystem`. Tasks 12–14 wire the CLI subcommands. Tasks 15–16 add an end-to-end integration test on real disk and tag `phase-1-complete`. The whole phase should land in 2–3 days of focused work.

**Repository state at start:** Working tree clean. `main` at `19b5394` (MSRV-1.85 bump). Phase 0 + 0.5 cleanup done. CI green on origin. Test count: 46.

**Important Phase 0/0.5 invariants this plan honors:**
- `Filesystem` trait uses `&self` throughout — never write `let mut fs = ...` in tests; never declare trait methods `&mut self`.
- `Filesystem::write(path, contents)` creates missing parent dirs by contract; do not call `create_dir_all` redundantly before every `write`.
- `Filesystem::exists` returns `io::Result<bool>` — always `.unwrap()` in tests (no `assert_eq!(..., bool)` patterns; clippy `bool_assert_comparison` will reject them — use `assert!(...)` / `assert!(!...)`).
- Use `std::io::Error::other("msg")` not `Error::new(ErrorKind::Other, "msg")` — clippy `io_other_error` will reject the latter.
- `MockFilesystem::symlink_metadata` is the TOCTOU-free way to detect whether a project path is already an aenv-managed symlink. Use it in activation logic, not `metadata` + `is_symlink`.
- `MockFilesystem::fail_writes_to(path)` / `fail_stats_on(path)` exist for rollback testing — use them rather than fighting the mock to produce errors.
- All paths below the CLI layer are absolute. The library never reads `std::env::current_dir()` or `std::env::var(...)`. Path resolution happens in `aenv-cli::main` and gets passed in.
- `AenvError` variants are locked. New failure paths must map to existing variants (`ActivationConflict`, `ManifestInvalid`, `NamespaceNotFound`, etc.) — do not add new variants in Phase 1.
- Tests should anticipate rustfmt `max_width = 100`. Pre-format multi-arg calls / long string literals.

---

## File structure (created in this phase)

**Library (`crates/aenv-core/src/`):**

| File | Responsibility |
|---|---|
| `home.rs` | `AENV_HOME` resolution and registry-directory layout (registry root, adapter dir, namespace dirs) |
| `manifest.rs` | Parse `aenv.toml` into `AenvManifest`; serialize a default for `aenv create` |
| `adapter.rs` | Parse adapter TOML into `Adapter`; the `AdapterRegistry` holds the loaded set |
| `adapters_builtin/mod.rs` | Embed built-in adapters via `include_str!`; write them to disk on first run |
| `adapters_builtin/claude_code.toml` | The claude-code adapter (the only one in Phase 1) |
| `namespace.rs` | Registry ops: `create_namespace`, `list_namespaces`, `delete_namespace` (all take a registry root) |
| `project.rs` | `.aenv` pin file IO and project-root resolution via ancestor walk |
| `state.rs` | `ActivationState` struct + serde for `.aenv/state.json` (with `schema_version: 1`) |
| `atomicity.rs` | The rename-probe (engineering §7) — verifies `.aenv/` is on the same filesystem as the project |
| `activate.rs` | `activate_namespace` — materialize the resolved file list, backup displaced files, write state, rollback on failure |
| `deactivate.rs` | `deactivate_namespace` — remove materialized files, restore backups, delete state |
| `restore.rs` | `restore_latest_backup` — independent of activation; restores from the most recent backup set |

**Library (modified):**

- `crates/aenv-core/src/lib.rs` — re-export new public types

**Binary (`crates/aenv-cli/src/`):**

| File | Responsibility |
|---|---|
| `main.rs` (modify) | Add the `Command` enum dispatched from clap |
| `paths.rs` | Resolve `AENV_HOME` (env var or default) and `--project` (or ancestor walk) into absolute paths |
| `cmd/mod.rs` | Module aggregator |
| `cmd/create.rs` | `aenv create <name>` |
| `cmd/list.rs` | `aenv list` (text only — `--json` lands in Phase 5) |
| `cmd/delete.rs` | `aenv delete <name>` (refuses if active in any tracked project) |
| `cmd/use_.rs` | `aenv use <name>` writes `.aenv` pin |
| `cmd/activate.rs` | `aenv activate [<name>] [--project <path>]` |
| `cmd/deactivate.rs` | `aenv deactivate [--project <path>]` |
| `cmd/restore.rs` | `aenv restore [--project <path>]` |
| `cmd/status.rs` | `aenv status [--project <path>]` (text only) |
| `cmd/adapter.rs` | `aenv adapter add <path>` + `aenv adapter list` |

**Tests (new):**

- `crates/aenv-core/tests/manifest.rs`
- `crates/aenv-core/tests/adapter.rs`
- `crates/aenv-core/tests/home.rs`
- `crates/aenv-core/tests/namespace.rs`
- `crates/aenv-core/tests/project.rs`
- `crates/aenv-core/tests/state.rs`
- `crates/aenv-core/tests/atomicity.rs`
- `crates/aenv-core/tests/activate.rs` (mock-driven, including failure-injection rollback)
- `crates/aenv-core/tests/deactivate.rs`
- `crates/aenv-core/tests/restore.rs`
- `crates/aenv-cli/tests/cli_e2e.rs` (real `tempdir`, drives the binary as a subprocess)

---

## Prerequisites

- [ ] **Step P1: Re-add `thiserror` to `aenv-cli/Cargo.toml`**

Phase 0.5 dropped it as unused; Phase 1's CLI error mapping will use it.

```toml
[dependencies]
aenv-core = { path = "../aenv-core" }
clap = { workspace = true }
thiserror = { workspace = true }
```

Replace the comment `# thiserror lands when ...` with the active line above.

- [ ] **Step P2: Verify Phase 0/0.5 baseline still green**

```bash
. "$HOME/.cargo/env"
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: all four exit 0; 46 tests pass.

No commit yet — the `thiserror` re-add lands in Task 14 (CLI wiring) where it's first used.

---

## Task 1: `AENV_HOME` resolution and registry-directory layout

**Files:**
- Create: `crates/aenv-core/src/home.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/home.rs`

**Purpose:** A single source of truth for "where do namespaces live on disk?" The library accepts a registry root as an absolute path; the CLI layer (Task 13) resolves `AENV_HOME` from the env var. This task builds the library-side struct + path helpers; CLI integration is later.

- [ ] **Step 1.1: Write the failing test**

Create `crates/aenv-core/tests/home.rs`:

```rust
//! Tests for `RegistryLayout`: derived paths under a registry root.

use aenv_core::home::RegistryLayout;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

#[test]
fn namespaces_dir_is_envs_subfolder() {
    assert_eq!(layout().namespaces_dir(), PathBuf::from("/aenv/envs"));
}

#[test]
fn namespace_dir_joins_under_envs() {
    assert_eq!(
        layout().namespace_dir("experiments"),
        PathBuf::from("/aenv/envs/experiments")
    );
}

#[test]
fn manifest_path_is_namespace_aenv_toml() {
    assert_eq!(
        layout().manifest_path("experiments"),
        PathBuf::from("/aenv/envs/experiments/aenv.toml")
    );
}

#[test]
fn adapters_dir_is_adapters_subfolder() {
    assert_eq!(layout().adapters_dir(), PathBuf::from("/aenv/adapters"));
}

#[test]
fn config_path_is_root_config_toml() {
    assert_eq!(
        layout().config_path(),
        PathBuf::from("/aenv/config.toml")
    );
}
```

- [ ] **Step 1.2: Run test (red)**

```bash
. "$HOME/.cargo/env"
cargo test --package aenv-core --test home
```

Expected: FAIL — `aenv_core::home` does not exist.

- [ ] **Step 1.3: Implement `RegistryLayout`**

Create `crates/aenv-core/src/home.rs`:

```rust
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
```

- [ ] **Step 1.4: Wire into `lib.rs`**

Modify `crates/aenv-core/src/lib.rs` to add the module:

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
pub mod home;

pub use error::{AenvError, Result};
```

- [ ] **Step 1.5: Run test (green)**

```bash
cargo test --package aenv-core --test home
```

Expected: all 5 tests pass.

- [ ] **Step 1.6: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add RegistryLayout: derived paths under AENV_HOME

Single source of truth for where namespaces, adapters, and config live
under the registry root. The CLI layer resolves AENV_HOME (env var or
default ~/.aenv) into an absolute path; this type takes that path and
computes everything else. Library layer never reads env vars per
engineering §6.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Manifest parsing (`aenv.toml`)

**Files:**
- Create: `crates/aenv-core/src/manifest.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/manifest.rs`

**Purpose:** Parse a namespace's `aenv.toml` into a typed struct. Phase 1 only consumes `name` and `[adapters.<name>]` tables; the `extends`, `[parameters]`, `[policies]`, and `[[skills]]` fields are parsed but recorded as Phase 2/3/4 placeholders. Invalid manifests map to `AenvError::ManifestInvalid` (exit 12).

- [ ] **Step 2.1: Write the failing test**

Create `crates/aenv-core/tests/manifest.rs`:

```rust
//! Tests for `aenv.toml` parsing.

use aenv_core::manifest::{AdapterEntry, AenvManifest};
use aenv_core::AenvError;

#[test]
fn parses_minimal_manifest_with_one_adapter() {
    let toml = r#"
        name = "experiments"

        [adapters.claude-code]
        files = ["CLAUDE.md"]
    "#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.name, "experiments");
    assert_eq!(m.extends, Vec::<String>::new());
    assert_eq!(m.adapters.len(), 1);
    let claude = m.adapters.get("claude-code").unwrap();
    assert_eq!(claude.files, vec!["CLAUDE.md".to_string()]);
}

#[test]
fn parses_extends_list_when_present() {
    let toml = r#"
        name = "detailed-execution"
        extends = ["base"]

        [adapters.claude-code]
        files = ["CLAUDE.md", ".claude/"]
    "#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.extends, vec!["base".to_string()]);
}

#[test]
fn parses_multiple_adapters() {
    let toml = r#"
        name = "experiments"

        [adapters.claude-code]
        files = ["CLAUDE.md"]

        [adapters.cursor]
        files = [".cursorrules"]
    "#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.adapters.len(), 2);
}

#[test]
fn rejects_missing_name() {
    let toml = r#"
        [adapters.claude-code]
        files = ["CLAUDE.md"]
    "#;
    let err = AenvManifest::from_toml(toml).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
    assert_eq!(err.exit_code(), 12);
}

#[test]
fn rejects_malformed_toml() {
    let toml = r#"name = "experiments" this is not valid toml"#;
    let err = AenvManifest::from_toml(toml).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn empty_adapters_table_is_valid() {
    // A namespace with no adapters declares no managed files. Valid but
    // useless; activation will just be a no-op.
    let toml = r#"name = "empty""#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.name, "empty");
    assert!(m.adapters.is_empty());
}

#[test]
fn round_trip_default_manifest() {
    // `aenv create <name>` writes a default manifest; parsing it back must
    // produce the same logical content.
    let toml = AenvManifest::default_for("experiments").to_toml();
    let m = AenvManifest::from_toml(&toml).unwrap();
    assert_eq!(m.name, "experiments");
    assert!(m.adapters.is_empty());
    assert!(m.extends.is_empty());
}

#[test]
fn adapter_entry_default_files_is_empty() {
    // Backstop: an adapter with no `files` key parses as having no files.
    let toml = r#"
        name = "experiments"

        [adapters.claude-code]
    "#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let claude = m.adapters.get("claude-code").unwrap();
    assert_eq!(claude.files, Vec::<String>::new());
}

#[test]
fn adapter_entry_fields_are_publicly_constructible() {
    // Compile-time check: AdapterEntry's fields stay pub. Downstream
    // consumers (Phase 2's composition layer) build these directly.
    let entry = AdapterEntry {
        files: vec!["CLAUDE.md".to_string()],
    };
    assert_eq!(entry.files.len(), 1);
}
```

- [ ] **Step 2.2: Run test (red)**

```bash
cargo test --package aenv-core --test manifest
```

Expected: FAIL — `aenv_core::manifest` does not exist.

- [ ] **Step 2.3: Implement `AenvManifest`**

Create `crates/aenv-core/src/manifest.rs`:

```rust
//! Namespace manifest (`aenv.toml`) parsing.
//!
//! Phase 1 consumes only `name`, `extends`, and `[adapters.<name>]` tables.
//! Forward-compat fields (`[parameters]`, `[policies]`, `[[skills]]`,
//! `[[agents]]`) are accepted but not yet parsed into typed values — they
//! land in Phases 3 and 4.

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A parsed namespace manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AenvManifest {
    /// Namespace name (must match the directory name; checked at activation time).
    pub name: String,

    /// Parent namespaces to inherit from. Empty in Phase 1; resolution lands in Phase 2.
    #[serde(default)]
    pub extends: Vec<String>,

    /// Per-adapter configuration. Keys are adapter names (e.g. "claude-code").
    #[serde(default)]
    pub adapters: BTreeMap<String, AdapterEntry>,
}

/// Per-adapter manifest entry: which files the adapter manages for this namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterEntry {
    /// Project-relative paths the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
}

impl AenvManifest {
    /// Parse a manifest from a TOML string. Returns `ManifestInvalid` on any
    /// parse error or missing required field.
    pub fn from_toml(input: &str) -> Result<Self> {
        let manifest: AenvManifest = toml::from_str(input)
            .map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;
        Ok(manifest)
    }

    /// Render the manifest to a canonical TOML string.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("AenvManifest serialization is infallible")
    }

    /// Build the manifest `aenv create <name>` writes by default — just the
    /// name, no adapters, no extends. Users add adapters by editing the file.
    pub fn default_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            extends: Vec::new(),
            adapters: BTreeMap::new(),
        }
    }
}
```

- [ ] **Step 2.4: Wire into `lib.rs`**

Modify `crates/aenv-core/src/lib.rs`:

```rust
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;

pub use error::{AenvError, Result};
```

- [ ] **Step 2.5: Run test (green)**

```bash
cargo test --package aenv-core --test manifest
```

Expected: 9 tests pass.

- [ ] **Step 2.6: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add AenvManifest parsing for aenv.toml

Phase 1 consumes only name, extends, and [adapters.<name>] tables.
Forward-compat fields (parameters/policies/skills/agents) are accepted
silently and round-trip through serde without being type-checked yet.
Invalid TOML or missing fields map to AenvError::ManifestInvalid
(exit 12).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Adapter parsing + claude-code built-in

**Files:**
- Create: `crates/aenv-core/src/adapter.rs`
- Create: `crates/aenv-core/src/adapters_builtin/mod.rs`
- Create: `crates/aenv-core/src/adapters_builtin/claude_code.toml`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/adapter.rs`

**Purpose:** Adapters describe a tool's config files and merge strategies. Phase 1 ships one built-in (claude-code), embeds it via `include_str!`, and writes it to disk on first run. `AdapterRegistry` is the in-memory loaded set; the parser is the entry point.

- [ ] **Step 3.1: Write the failing test**

Create `crates/aenv-core/tests/adapter.rs`:

```rust
//! Tests for adapter TOML parsing and the built-in registry.

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::adapters_builtin;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::AenvError;
use std::path::PathBuf;

#[test]
fn parses_minimal_adapter() {
    let toml = r#"
        name = "claude-code"
        files = ["CLAUDE.md", ".claude/"]
    "#;
    let a = Adapter::from_toml(toml).unwrap();
    assert_eq!(a.name, "claude-code");
    assert_eq!(a.files, vec!["CLAUDE.md".to_string(), ".claude/".to_string()]);
}

#[test]
fn rejects_missing_name() {
    let toml = r#"files = ["CLAUDE.md"]"#;
    let err = Adapter::from_toml(toml).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn rejects_malformed_toml() {
    let toml = r#"name = ::: nope"#;
    let err = Adapter::from_toml(toml).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn registry_starts_empty() {
    let reg = AdapterRegistry::new();
    assert!(reg.get("anything").is_none());
    assert_eq!(reg.len(), 0);
}

#[test]
fn registry_insert_then_lookup() {
    let mut reg = AdapterRegistry::new();
    let a = Adapter {
        name: "claude-code".to_string(),
        files: vec!["CLAUDE.md".to_string()],
    };
    reg.insert(a.clone());
    assert_eq!(reg.get("claude-code"), Some(&a));
    assert_eq!(reg.len(), 1);
}

#[test]
fn builtin_claude_code_parses() {
    // The embedded claude-code adapter must itself be valid TOML.
    let toml = adapters_builtin::CLAUDE_CODE_TOML;
    let a = Adapter::from_toml(toml).expect("embedded claude-code must parse");
    assert_eq!(a.name, "claude-code");
    assert!(a.files.iter().any(|f| f == "CLAUDE.md"));
}

#[test]
fn install_builtins_writes_claude_code_to_disk() {
    let fs = MockFilesystem::new();
    let adapters_dir = PathBuf::from("/aenv/adapters");
    adapters_builtin::install_builtins(&fs, &adapters_dir).unwrap();
    let written = fs.read(&adapters_dir.join("claude-code.toml")).unwrap();
    let parsed = Adapter::from_toml(&String::from_utf8(written).unwrap()).unwrap();
    assert_eq!(parsed.name, "claude-code");
}

#[test]
fn install_builtins_is_idempotent_for_unchanged_files() {
    // If the file already exists with identical content, install_builtins
    // leaves it alone (no rewrite). This matters because we re-run on every
    // CLI invocation in Task 13.
    let fs = MockFilesystem::new();
    let adapters_dir = PathBuf::from("/aenv/adapters");
    adapters_builtin::install_builtins(&fs, &adapters_dir).unwrap();
    adapters_builtin::install_builtins(&fs, &adapters_dir).unwrap();
    // Read it back — still claude-code.
    let parsed = Adapter::from_toml(
        &String::from_utf8(fs.read(&adapters_dir.join("claude-code.toml")).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(parsed.name, "claude-code");
}

#[test]
fn install_builtins_does_not_overwrite_user_modified_file() {
    // If a user has edited their copy of claude-code.toml, install_builtins
    // must not clobber it. Engineering doc §4: "Users can override [built-ins]
    // by writing a same-named adapter file; the user file wins."
    let fs = MockFilesystem::new();
    let adapters_dir = PathBuf::from("/aenv/adapters");
    let path = adapters_dir.join("claude-code.toml");
    let user_content = b"name = \"claude-code\"\nfiles = [\"only-this.md\"]\n";
    fs.write(&path, user_content).unwrap();

    adapters_builtin::install_builtins(&fs, &adapters_dir).unwrap();

    assert_eq!(fs.read(&path).unwrap(), user_content);
}

#[test]
fn load_adapters_dir_reads_all_files() {
    let fs = MockFilesystem::new();
    let dir = PathBuf::from("/aenv/adapters");
    fs.write(
        &dir.join("claude-code.toml"),
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    fs.write(
        &dir.join("cursor.toml"),
        b"name = \"cursor\"\nfiles = [\".cursorrules\"]\n",
    )
    .unwrap();

    let reg = AdapterRegistry::load_from_dir(&fs, &dir).unwrap();
    assert_eq!(reg.len(), 2);
    assert!(reg.get("claude-code").is_some());
    assert!(reg.get("cursor").is_some());
}

#[test]
fn load_adapters_dir_skips_non_toml_files() {
    let fs = MockFilesystem::new();
    let dir = PathBuf::from("/aenv/adapters");
    fs.write(
        &dir.join("claude-code.toml"),
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    fs.write(&dir.join("README"), b"not a toml file\n").unwrap();

    let reg = AdapterRegistry::load_from_dir(&fs, &dir).unwrap();
    assert_eq!(reg.len(), 1);
}
```

- [ ] **Step 3.2: Run test (red)**

```bash
cargo test --package aenv-core --test adapter
```

Expected: FAIL — `aenv_core::adapter` does not exist.

- [ ] **Step 3.3: Implement `Adapter` and `AdapterRegistry`**

Create `crates/aenv-core/src/adapter.rs`:

```rust
//! Adapter TOML parsing and registry.
//!
//! An adapter declares a tool's project-relative paths and (in Phase 2)
//! merge strategies. Phase 1 supports parsing the minimal `name` + `files`
//! fields; merge strategies are accepted via serde's default but unused.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// A parsed adapter definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adapter {
    /// Adapter name (e.g. "claude-code").
    pub name: String,
    /// Project-relative paths or directory prefixes the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
    /// Merge strategies keyed by relative path. Unused in Phase 1.
    #[serde(default)]
    pub merge_strategies: BTreeMap<String, String>,
}

impl Adapter {
    /// Parse an adapter from a TOML string.
    pub fn from_toml(input: &str) -> Result<Self> {
        toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))
    }
}

/// In-memory set of loaded adapters, keyed by name.
#[derive(Debug, Default, Clone)]
pub struct AdapterRegistry {
    adapters: BTreeMap<String, Adapter>,
}

impl AdapterRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of adapters loaded.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Add an adapter to the registry.
    pub fn insert(&mut self, adapter: Adapter) {
        self.adapters.insert(adapter.name.clone(), adapter);
    }

    /// Look up an adapter by name.
    pub fn get(&self, name: &str) -> Option<&Adapter> {
        self.adapters.get(name)
    }

    /// Iterate over all adapters.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Adapter)> {
        self.adapters.iter()
    }

    /// Load every `.toml` file from `dir` into a registry. Non-TOML files
    /// are silently skipped. A missing `dir` returns an empty registry.
    pub fn load_from_dir<F: Filesystem>(fs: &F, dir: &Path) -> Result<Self> {
        let mut reg = Self::new();
        if !fs.exists(dir)? {
            return Ok(reg);
        }
        for path in fs.list_dir(dir)? {
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            let bytes = fs.read(&path)?;
            let toml_str = std::str::from_utf8(&bytes)
                .map_err(|e| AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", path.display())))?;
            reg.insert(Adapter::from_toml(toml_str)?);
        }
        Ok(reg)
    }
}
```

- [ ] **Step 3.4: Create the claude-code adapter TOML**

Create `crates/aenv-core/src/adapters_builtin/claude_code.toml`:

```toml
name = "claude-code"
files = ["CLAUDE.md", ".claude/"]
```

- [ ] **Step 3.5: Implement the built-in install logic**

Create `crates/aenv-core/src/adapters_builtin/mod.rs`:

```rust
//! Built-in adapters embedded into the binary.
//!
//! Engineering §4: "Built-in adapters ship as embedded TOML strings via
//! `include_str!` and are written to disk on first run. Users can override
//! them by writing a same-named adapter file; the user file wins."

use crate::error::Result;
use crate::fs::Filesystem;
use std::path::Path;

/// The claude-code adapter, embedded at compile time.
pub const CLAUDE_CODE_TOML: &str = include_str!("claude_code.toml");

/// Every built-in adapter as a (filename, contents) pair.
const BUILTINS: &[(&str, &str)] = &[("claude-code.toml", CLAUDE_CODE_TOML)];

/// Write any built-in adapter that isn't already present on disk into
/// `adapters_dir`. Existing files are left untouched — even if their
/// contents differ from the embedded version — so that a user who has
/// edited their copy keeps their changes.
pub fn install_builtins<F: Filesystem>(fs: &F, adapters_dir: &Path) -> Result<()> {
    fs.create_dir_all(adapters_dir)?;
    for (filename, contents) in BUILTINS {
        let target = adapters_dir.join(filename);
        if fs.exists(&target)? {
            continue;
        }
        fs.write(&target, contents.as_bytes())?;
    }
    Ok(())
}
```

- [ ] **Step 3.6: Wire into `lib.rs`**

Modify `crates/aenv-core/src/lib.rs`:

```rust
pub mod adapter;
pub mod adapters_builtin;
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;

pub use error::{AenvError, Result};
```

- [ ] **Step 3.7: Run tests (green)**

```bash
cargo test --package aenv-core --test adapter
```

Expected: 11 tests pass.

- [ ] **Step 3.8: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add Adapter parsing + AdapterRegistry + claude-code built-in

Adapters are pure data: name + files + (future) merge_strategies. The
claude-code TOML is embedded via include_str! and written to disk on
first run via install_builtins(). User edits are preserved — an
existing file is never overwritten, matching the engineering doc §4
contract that "the user file wins."

AdapterRegistry::load_from_dir walks the adapters directory, parses
every *.toml file, and silently skips non-TOML entries. A missing
directory returns an empty registry rather than erroring.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Namespace registry operations

**Files:**
- Create: `crates/aenv-core/src/namespace.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/namespace.rs`

**Purpose:** The library-side `create_namespace`, `list_namespaces`, `delete_namespace` operations. These take a `RegistryLayout` + `Filesystem` and produce/inspect the on-disk namespace directories.

- [ ] **Step 4.1: Write the failing test**

Create `crates/aenv-core/tests/namespace.rs`:

```rust
//! Tests for namespace registry operations.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::namespace::{create_namespace, delete_namespace, list_namespaces};
use aenv_core::AenvError;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

#[test]
fn create_writes_default_manifest() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();

    let manifest_bytes = fs.read(&layout.manifest_path("experiments")).unwrap();
    let m = AenvManifest::from_toml(&String::from_utf8(manifest_bytes).unwrap()).unwrap();
    assert_eq!(m.name, "experiments");
    assert!(m.adapters.is_empty());
}

#[test]
fn create_rejects_duplicate() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();
    let err = create_namespace(&fs, &layout, "experiments").expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn list_returns_empty_when_no_namespaces() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert!(names.is_empty());
}

#[test]
fn list_returns_namespace_names_sorted() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();
    create_namespace(&fs, &layout, "analyst").unwrap();
    create_namespace(&fs, &layout, "detailed-execution").unwrap();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert_eq!(
        names,
        vec![
            "analyst".to_string(),
            "detailed-execution".to_string(),
            "experiments".to_string(),
        ]
    );
}

#[test]
fn list_skips_entries_without_manifest() {
    // A stray directory under envs/ that lacks aenv.toml is not a namespace.
    // list_namespaces silently ignores it.
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "real").unwrap();
    fs.create_dir_all(&layout.namespaces_dir().join("stray"))
        .unwrap();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert_eq!(names, vec!["real".to_string()]);
}

#[test]
fn delete_removes_namespace_directory() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();
    delete_namespace(&fs, &layout, "experiments").unwrap();
    assert!(!fs.exists(&layout.namespace_dir("experiments")).unwrap());
}

#[test]
fn delete_rejects_unknown_namespace() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let err = delete_namespace(&fs, &layout, "nope").expect_err("must error");
    assert!(matches!(err, AenvError::NamespaceNotFound(_)));
    assert_eq!(err.exit_code(), 10);
}
```

- [ ] **Step 4.2: Run test (red)**

```bash
cargo test --package aenv-core --test namespace
```

Expected: FAIL — `aenv_core::namespace` does not exist.

- [ ] **Step 4.3: Implement namespace operations**

Create `crates/aenv-core/src/namespace.rs`:

```rust
//! Namespace registry operations: create, list, delete.
//!
//! These are pure library operations against a `Filesystem` and a
//! `RegistryLayout`. The CLI layer wires them to `aenv create / list /
//! delete`. Each function takes absolute paths via the layout — no env-var
//! reads, no current-dir reads.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::manifest::AenvManifest;

/// Create a new namespace by writing a default manifest. Errors if a
/// manifest already exists for `name` (PRD R-5).
pub fn create_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    name: &str,
) -> Result<()> {
    let manifest_path = layout.manifest_path(name);
    if fs.exists(&manifest_path)? {
        return Err(AenvError::ManifestInvalid(format!(
            "namespace '{name}' already exists"
        )));
    }
    let manifest = AenvManifest::default_for(name);
    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    Ok(())
}

/// List every namespace in the registry. A namespace is any directory
/// under `envs/` that contains an `aenv.toml`. Returns names sorted
/// lexicographically.
pub fn list_namespaces<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
) -> Result<Vec<String>> {
    let envs_dir = layout.namespaces_dir();
    if !fs.exists(&envs_dir)? {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs.list_dir(&envs_dir)? {
        let name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        let Some(name) = name else { continue };
        if fs.exists(&layout.manifest_path(&name))? {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Delete a namespace. Errors if the namespace does not exist
/// (`NamespaceNotFound`, exit 10).
///
/// Note: PRD R-4 requires checking that the namespace is not currently
/// active in any tracked project. Phase 1 lacks a project-tracking
/// registry, so this safety net is best-effort — the CLI layer will warn
/// users that delete is destructive.
pub fn delete_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    name: &str,
) -> Result<()> {
    let dir = layout.namespace_dir(name);
    if !fs.exists(&dir)? {
        return Err(AenvError::NamespaceNotFound(name.to_string()));
    }
    fs.remove_dir_all(&dir)?;
    Ok(())
}
```

- [ ] **Step 4.4: Wire into `lib.rs`**

```rust
pub mod adapter;
pub mod adapters_builtin;
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;
pub mod namespace;

pub use error::{AenvError, Result};
```

- [ ] **Step 4.5: Run tests (green)**

```bash
cargo test --package aenv-core --test namespace
```

Expected: 7 tests pass.

- [ ] **Step 4.6: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add create/list/delete namespace registry operations

Pure library ops against a Filesystem + RegistryLayout. create rejects
duplicates as ManifestInvalid (exit 12) — PRD R-5. list returns names
sorted, silently skipping any directory under envs/ that lacks an
aenv.toml. delete errors NamespaceNotFound (exit 10) on a missing
namespace.

The PRD R-4 safety check ("namespace not currently active in any
tracked project") is best-effort in Phase 1 — we don't yet maintain a
project-tracking registry. The CLI layer (Task 14) will warn users.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Project pin file and root resolution

**Files:**
- Create: `crates/aenv-core/src/project.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/project.rs`

**Purpose:** Read/write `.aenv` pin files at the project root. Walk up the directory tree looking for `.aenv` to identify the project root from any subdirectory.

- [ ] **Step 5.1: Write the failing test**

Create `crates/aenv-core/tests/project.rs`:

```rust
//! Tests for `.aenv` pin file IO and project-root resolution.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::project::{find_project_root, read_pin, write_pin};
use aenv_core::AenvError;
use std::path::{Path, PathBuf};

#[test]
fn write_then_read_pin_roundtrip() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/payments-api");
    write_pin(&fs, &project, "detailed-execution").unwrap();
    let pin = read_pin(&fs, &project).unwrap();
    assert_eq!(pin, "detailed-execution");
}

#[test]
fn read_pin_errors_when_missing() {
    let fs = MockFilesystem::new();
    let err = read_pin(&fs, Path::new("/projects/missing"))
        .expect_err("must error");
    assert!(matches!(err, AenvError::ProjectNotPinned));
    assert_eq!(err.exit_code(), 20);
}

#[test]
fn read_pin_strips_trailing_whitespace() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/p");
    fs.write(&project.join(".aenv"), b"experiments\n").unwrap();
    let pin = read_pin(&fs, &project).unwrap();
    assert_eq!(pin, "experiments");
}

#[test]
fn read_pin_rejects_blank_content() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/p");
    fs.write(&project.join(".aenv"), b"   \n\n").unwrap();
    let err = read_pin(&fs, &project).expect_err("must error");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn read_pin_takes_first_non_blank_line() {
    // R-33: ".aenv file at a project root containing one namespace name
    // per line." Phase 1 supports only single-namespace pin, so we take
    // the first non-blank line and ignore the rest with a warning later.
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/p");
    fs.write(&project.join(".aenv"), b"experiments\n# comment\n")
        .unwrap();
    let pin = read_pin(&fs, &project).unwrap();
    assert_eq!(pin, "experiments");
}

#[test]
fn find_project_root_returns_self_when_pin_present() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/payments-api");
    fs.write(&project.join(".aenv"), b"experiments\n").unwrap();
    let root = find_project_root(&fs, &project).unwrap();
    assert_eq!(root, project);
}

#[test]
fn find_project_root_walks_up_to_ancestor() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/payments-api");
    fs.write(&project.join(".aenv"), b"experiments\n").unwrap();
    let nested = project.join("src/handlers");
    fs.create_dir_all(&nested).unwrap();
    let root = find_project_root(&fs, &nested).unwrap();
    assert_eq!(root, project);
}

#[test]
fn find_project_root_returns_err_when_no_ancestor_pinned() {
    let fs = MockFilesystem::new();
    let nested = PathBuf::from("/tmp/wherever/deep/path");
    fs.create_dir_all(&nested).unwrap();
    let err = find_project_root(&fs, &nested).expect_err("must error");
    assert!(matches!(err, AenvError::ProjectNotPinned));
}

#[test]
fn find_project_root_prefers_nearest_pin_ancestor() {
    // Per functional spec §9 "Nested projects": the nearest-ancestor
    // .aenv wins.
    let fs = MockFilesystem::new();
    let monorepo = PathBuf::from("/projects/monorepo");
    let inner = monorepo.join("experiments");
    fs.write(&monorepo.join(".aenv"), b"detailed-execution\n").unwrap();
    fs.write(&inner.join(".aenv"), b"experiments\n").unwrap();
    let root = find_project_root(&fs, &inner).unwrap();
    assert_eq!(root, inner);
}
```

- [ ] **Step 5.2: Run test (red)**

```bash
cargo test --package aenv-core --test project
```

Expected: FAIL — `aenv_core::project` does not exist.

- [ ] **Step 5.3: Implement project module**

Create `crates/aenv-core/src/project.rs`:

```rust
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
pub fn write_pin<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    namespace_name: &str,
) -> Result<()> {
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
    let text = String::from_utf8(bytes).map_err(|e| {
        AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", path.display()))
    })?;
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
pub fn find_project_root<F: Filesystem>(fs: &F, start: &Path) -> Result<PathBuf> {
    let mut current: Option<&Path> = Some(start);
    while let Some(dir) = current {
        if fs.exists(&dir.join(PIN_FILENAME))? {
            return Ok(dir.to_path_buf());
        }
        current = dir.parent();
    }
    Err(AenvError::ProjectNotPinned)
}
```

- [ ] **Step 5.4: Wire into `lib.rs`**

```rust
pub mod adapter;
pub mod adapters_builtin;
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;
pub mod namespace;
pub mod project;

pub use error::{AenvError, Result};
```

- [ ] **Step 5.5: Run tests (green)**

```bash
cargo test --package aenv-core --test project
```

Expected: 9 tests pass.

- [ ] **Step 5.6: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add .aenv pin file IO and project-root resolution

write_pin / read_pin own the on-disk format: one namespace name per
line, # comments allowed, blank lines skipped. read_pin returns the
first non-blank/non-comment line — Phase 1 supports a single pin only.

find_project_root walks ancestors from `start` looking for the
nearest .aenv. Matches functional spec §9 "Nested projects: the
nearest-ancestor .aenv wins."

Missing pin -> ProjectNotPinned (exit 20). Blank/non-utf-8 content ->
ManifestInvalid (exit 12).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Activation state file schema

**Files:**
- Create: `crates/aenv-core/src/state.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/state.rs`

**Purpose:** Serialize and deserialize `.aenv/state.json`. Schema starts at version 1; a state file with a higher `schema_version` returns an error so older binaries don't mishandle newer state (engineering §11).

- [ ] **Step 6.1: Write the failing test**

Create `crates/aenv-core/tests/state.rs`:

```rust
//! Tests for ActivationState serialization.

use aenv_core::state::{ActivationState, BackedUpFile, ManagedFile, MaterializeStrategy};
use aenv_core::AenvError;
use std::path::PathBuf;

fn sample_state() -> ActivationState {
    ActivationState {
        schema_version: 1,
        active_namespace: "experiments".to_string(),
        project_root: PathBuf::from("/projects/p"),
        managed_files: vec![ManagedFile {
            path: PathBuf::from("CLAUDE.md"),
            strategy: MaterializeStrategy::Symlink,
            source: Some(PathBuf::from("/aenv/envs/experiments/CLAUDE.md")),
        }],
        backed_up: vec![BackedUpFile {
            original_path: PathBuf::from("CLAUDE.md"),
            backup_path: PathBuf::from(".aenv/backup/2026-05-20T14-22-03/CLAUDE.md"),
        }],
    }
}

#[test]
fn round_trip_via_json() {
    let state = sample_state();
    let json = state.to_json().unwrap();
    let parsed = ActivationState::from_json(&json).unwrap();
    assert_eq!(parsed, state);
}

#[test]
fn rejects_unknown_higher_schema_version() {
    let json = r#"{
        "schema_version": 999,
        "active_namespace": "x",
        "project_root": "/p",
        "managed_files": [],
        "backed_up": []
    }"#;
    let err = ActivationState::from_json(json).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
    assert!(err.to_string().contains("schema_version"));
}

#[test]
fn rejects_malformed_json() {
    let err = ActivationState::from_json("{ not json").expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn empty_state_has_no_managed_or_backed_up() {
    let json = r#"{
        "schema_version": 1,
        "active_namespace": "empty",
        "project_root": "/p",
        "managed_files": [],
        "backed_up": []
    }"#;
    let state = ActivationState::from_json(json).unwrap();
    assert_eq!(state.managed_files.len(), 0);
    assert_eq!(state.backed_up.len(), 0);
}

#[test]
fn serializes_strategy_as_lowercase_string() {
    let state = sample_state();
    let json = state.to_json().unwrap();
    assert!(json.contains("\"symlink\""));
}
```

- [ ] **Step 6.2: Run test (red)**

```bash
cargo test --package aenv-core --test state
```

Expected: FAIL — `aenv_core::state` does not exist.

- [ ] **Step 6.3: Implement `ActivationState`**

Create `crates/aenv-core/src/state.rs`:

```rust
//! Activation state file (`.aenv/state.json`).
//!
//! Persisted after a successful activation. Records the active namespace,
//! every file aenv materialized, every original it backed up, and a schema
//! version so older binaries can refuse to operate on newer state files
//! (engineering §11).

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current schema version. Bump when changing the on-disk shape.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Materialization strategy used for a single managed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterializeStrategy {
    /// File is a symlink into the namespace directory.
    Symlink,
    /// File is a copy of the namespace file (Windows fallback, Phase 7).
    Copy,
    /// Project file's bytes match the namespace's, so aenv left it in
    /// place rather than symlinking over it. At deactivate time we
    /// likewise leave it alone: it's the user's content (and also the
    /// namespace's), so removing it would surprise the user.
    Identical,
    /// File is a merged regular file (Phase 2).
    Merged,
}

/// One file managed by the current activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedFile {
    /// Project-relative path.
    pub path: PathBuf,
    /// How the file was materialized.
    pub strategy: MaterializeStrategy,
    /// Source path inside the registry (None for `Identical`/`Merged`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source: Option<PathBuf>,
}

/// A file aenv backed up before materializing on top of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackedUpFile {
    /// Project-relative path of the original.
    pub original_path: PathBuf,
    /// Project-relative path of the backup copy.
    pub backup_path: PathBuf,
}

/// Persisted state of an active namespace in a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationState {
    /// Schema version of this file.
    pub schema_version: u32,
    /// Name of the active namespace.
    pub active_namespace: String,
    /// Absolute path to the project root.
    pub project_root: PathBuf,
    /// Files this activation materialized.
    pub managed_files: Vec<ManagedFile>,
    /// Files this activation backed up before materializing over them.
    pub backed_up: Vec<BackedUpFile>,
}

impl ActivationState {
    /// Serialize to pretty JSON for on-disk storage.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AenvError::ManifestInvalid(format!("state serialization: {e}")))
    }

    /// Deserialize from JSON, rejecting any unknown future schema version.
    pub fn from_json(input: &str) -> Result<Self> {
        let state: ActivationState = serde_json::from_str(input)
            .map_err(|e| AenvError::ManifestInvalid(format!("state parse: {e}")))?;
        if state.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(AenvError::ManifestInvalid(format!(
                "state schema_version {} > supported {}; upgrade aenv",
                state.schema_version, CURRENT_SCHEMA_VERSION
            )));
        }
        Ok(state)
    }
}
```

- [ ] **Step 6.4: Add `serde_json` to `aenv-core/Cargo.toml`**

Modify `crates/aenv-core/Cargo.toml` `[dependencies]`:

```toml
[dependencies]
thiserror = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
```

- [ ] **Step 6.5: Wire into `lib.rs`**

```rust
pub mod adapter;
pub mod adapters_builtin;
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;
pub mod namespace;
pub mod project;
pub mod state;

pub use error::{AenvError, Result};
```

- [ ] **Step 6.6: Run tests (green)**

```bash
cargo test --package aenv-core --test state
```

Expected: 5 tests pass.

- [ ] **Step 6.7: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add ActivationState schema for .aenv/state.json

Records active namespace, every file aenv materialized (with strategy
+ source), and every original it backed up. Carries schema_version: 1;
loading a file with a higher version returns ManifestInvalid so older
binaries refuse to operate rather than mishandling unknown fields
(engineering §11 state file forward-compat).

MaterializeStrategy variants: Symlink, Copy, Identical, Merged. Phase 1
uses only Symlink and Identical; Copy is the Phase 7 Windows fallback;
Merged lands in Phase 2.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Atomicity probe

**Files:**
- Create: `crates/aenv-core/src/atomicity.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/atomicity.rs`

**Purpose:** Engineering §7 — verify that `.aenv/` is on the same filesystem as the project root by performing a rename within `.aenv/`. If the probe fails, abort activation with `ActivationConflict` (exit 13) so we never start an activation we can't atomically roll back.

- [ ] **Step 7.1: Write the failing test**

Create `crates/aenv-core/tests/atomicity.rs`:

```rust
//! Tests for the rename atomicity probe.

use aenv_core::atomicity::probe_rename_atomicity;
use aenv_core::fs::{Filesystem, MockFilesystem};
use std::path::PathBuf;

#[test]
fn probe_succeeds_on_clean_aenv_dir() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    probe_rename_atomicity(&fs, &project).unwrap();
}

#[test]
fn probe_creates_aenv_dir_if_absent() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    probe_rename_atomicity(&fs, &project).unwrap();
    assert!(fs.exists(&project.join(".aenv")).unwrap());
}

#[test]
fn probe_leaves_no_probe_files_behind() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    probe_rename_atomicity(&fs, &project).unwrap();
    let entries = fs.list_dir(&project.join(".aenv")).unwrap();
    // Probe should leave .aenv/ empty (or containing nothing it created).
    assert!(entries.is_empty(), "found leftover entries: {entries:?}");
}
```

- [ ] **Step 7.2: Run test (red)**

```bash
cargo test --package aenv-core --test atomicity
```

Expected: FAIL — `aenv_core::atomicity` does not exist.

- [ ] **Step 7.3: Implement the probe**

Create `crates/aenv-core/src/atomicity.rs`:

```rust
//! Rename atomicity probe (engineering §7).
//!
//! `std::fs::rename` is atomic on Unix *only when source and destination
//! are on the same filesystem*. If a project's `.aenv/` directory ends up
//! on a different mount (e.g. symlinked elsewhere), rename silently
//! degrades to copy+delete and we lose the atomicity guarantee R-45
//! depends on.
//!
//! The probe writes two tiny files inside `.aenv/`, renames one to the
//! other, and removes the survivor. If the rename succeeds the assumption
//! holds; failure surfaces as `ActivationConflict` (exit 13).

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use std::path::Path;

/// Run the probe. Creates `<project>/.aenv/` if it doesn't exist. Leaves
/// no probe files behind on success.
pub fn probe_rename_atomicity<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let aenv_dir = project_root.join(".aenv");
    fs.create_dir_all(&aenv_dir)?;

    let a = aenv_dir.join(".probe.a");
    let b = aenv_dir.join(".probe.b");

    // Cleanup any stale probe files from a previous interrupted run.
    let _ = fs.remove_file(&a);
    let _ = fs.remove_file(&b);

    fs.write(&a, b"probe").map_err(|e| {
        AenvError::ActivationConflict(format!("atomicity probe: write failed: {e}"))
    })?;
    fs.rename(&a, &b).map_err(|e| {
        // Clean up the source before bailing.
        let _ = fs.remove_file(&a);
        AenvError::ActivationConflict(format!("atomicity probe: rename failed: {e}"))
    })?;
    fs.remove_file(&b).map_err(|e| {
        AenvError::ActivationConflict(format!("atomicity probe: cleanup failed: {e}"))
    })?;

    Ok(())
}
```

- [ ] **Step 7.4: Wire into `lib.rs`**

```rust
pub mod adapter;
pub mod adapters_builtin;
pub mod atomicity;
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;
pub mod namespace;
pub mod project;
pub mod state;

pub use error::{AenvError, Result};
```

- [ ] **Step 7.5: Run tests (green)**

```bash
cargo test --package aenv-core --test atomicity
```

Expected: 3 tests pass.

- [ ] **Step 7.6: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add rename atomicity probe

Engineering §7: std::fs::rename is atomic on Unix only when source and
destination are on the same filesystem. The probe verifies that .aenv/
is on the same mount as the project by performing an in-.aenv/ rename
before any activation begins. Failure aborts with ActivationConflict
(exit 13). Probe cleans up after itself so .aenv/ ends up empty.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Activation — symlink case (no displaced files)

**Files:**
- Create: `crates/aenv-core/src/activate.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/activate.rs`

**Purpose:** Start with the simplest activation path: no displaced files. Adapter declares a file; namespace has that file; project doesn't. We symlink from project to namespace and record state.

This task introduces `activate_namespace` with one TDD cycle. Tasks 9 and 10 add the displaced-files and rollback variants.

- [ ] **Step 8.1: Write the failing test**

Create `crates/aenv-core/tests/activate.rs`:

```rust
//! Tests for the activation primitive `activate_namespace`.
//!
//! Mock-driven so we can exercise rollback paths via fail injection.
//! Real-filesystem end-to-end coverage lives in `aenv-cli/tests/cli_e2e.rs`.

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::create_namespace;
use aenv_core::state::{ActivationState, MaterializeStrategy};
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

fn claude_adapter() -> Adapter {
    Adapter {
        name: "claude-code".to_string(),
        files: vec!["CLAUDE.md".to_string()],
        merge_strategies: Default::default(),
    }
}

fn setup_registry_with_namespace(fs: &MockFilesystem, ns: &str, files: &[(&str, &[u8])]) {
    let layout = layout();
    create_namespace(fs, &layout, ns).unwrap();
    // Patch the manifest to reference claude-code so the adapter's files apply.
    let manifest = format!(
        "name = \"{ns}\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n"
    );
    fs.write(&layout.manifest_path(ns), manifest.as_bytes()).unwrap();
    for (rel, content) in files {
        fs.write(&layout.namespace_dir(ns).join(rel), content).unwrap();
    }
}

fn registry_with_claude() -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    r.insert(claude_adapter());
    r
}

#[test]
fn symlinks_new_file_into_project() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"disposition")]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();

    let state =
        activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
            .unwrap();

    // Project file is a symlink to the namespace file.
    assert!(fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
    assert_eq!(
        fs.read_link(&project.join("CLAUDE.md")).unwrap(),
        layout.namespace_dir("experiments").join("CLAUDE.md")
    );

    // State records exactly that.
    assert_eq!(state.active_namespace, "experiments");
    assert_eq!(state.managed_files.len(), 1);
    assert_eq!(state.managed_files[0].path, PathBuf::from("CLAUDE.md"));
    assert_eq!(
        state.managed_files[0].strategy,
        MaterializeStrategy::Symlink
    );
    assert!(state.backed_up.is_empty());

    // State file is persisted at .aenv/state.json.
    let on_disk = fs.read(&project.join(".aenv/state.json")).unwrap();
    let parsed = ActivationState::from_json(&String::from_utf8(on_disk).unwrap()).unwrap();
    assert_eq!(parsed, state);
}

#[test]
fn errors_when_namespace_does_not_exist() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = activate_namespace(&fs, &layout, &registry_with_claude(), &project, "missing")
        .expect_err("must error");
    assert!(matches!(err, aenv_core::AenvError::NamespaceNotFound(_)));
    assert_eq!(err.exit_code(), 10);
}

#[test]
fn errors_when_manifest_names_unknown_adapter() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();
    // Manifest names an adapter not in the registry.
    let manifest = "name = \"experiments\"\n\n[adapters.cursor]\nfiles = [\".cursorrules\"]\n";
    fs.write(&layout.manifest_path("experiments"), manifest.as_bytes())
        .unwrap();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();

    let err =
        activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
            .expect_err("must error");
    assert!(matches!(err, aenv_core::AenvError::AdapterMissing(_)));
    assert_eq!(err.exit_code(), 11);
}

#[test]
fn missing_adapter_file_is_skipped_silently() {
    // If the adapter declares CLAUDE.md but the namespace doesn't ship it,
    // nothing happens for that file. (No error: the adapter's files list
    // is what *might* be managed, not a guarantee every namespace ships it.)
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[]); // empty
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();

    let state =
        activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
            .unwrap();
    assert!(state.managed_files.is_empty());
}
```

- [ ] **Step 8.2: Run test (red)**

```bash
cargo test --package aenv-core --test activate
```

Expected: FAIL — `aenv_core::activate` does not exist.

- [ ] **Step 8.3: Implement `activate_namespace` (symlink case only)**

Create `crates/aenv-core/src/activate.rs`:

```rust
//! Activation: materialize a namespace's files into a project.
//!
//! Phase 1 supports one adapter at a time with the simplest set of cases:
//! file doesn't exist in project -> symlink; file exists and differs ->
//! back up then symlink (Task 9); file exists and is byte-identical ->
//! leave in place and mark managed (Task 9). Activation failure rolls
//! back any partial materialization (Task 10).

use crate::adapter::AdapterRegistry;
use crate::atomicity::probe_rename_atomicity;
use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::manifest::AenvManifest;
use crate::state::{ActivationState, ManagedFile, MaterializeStrategy, CURRENT_SCHEMA_VERSION};
use std::path::{Path, PathBuf};

/// Activate `namespace_name` into `project_root`. Writes a state file at
/// `<project>/.aenv/state.json` on success.
pub fn activate_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    namespace_name: &str,
) -> Result<ActivationState> {
    let manifest = load_manifest(fs, layout, namespace_name)?;

    // Every adapter named in the manifest must be installed.
    for adapter_name in manifest.adapters.keys() {
        if adapters.get(adapter_name).is_none() {
            return Err(AenvError::AdapterMissing(adapter_name.clone()));
        }
    }

    // Probe rename atomicity before doing anything irreversible.
    probe_rename_atomicity(fs, project_root)?;

    let mut managed_files = Vec::new();

    for (adapter_name, entry) in &manifest.adapters {
        let adapter = adapters.get(adapter_name).expect("checked above");
        for rel in adapter_files_for_entry(adapter, entry) {
            let source = layout.namespace_dir(namespace_name).join(&rel);
            // If the namespace doesn't ship this file, skip silently.
            if !fs.exists(&source)? {
                continue;
            }
            let project_path = project_root.join(&rel);
            // Phase 1: file doesn't exist in project -> symlink.
            // Phases 9/10 add the displaced + identical paths.
            fs.symlink(&source, &project_path)?;
            managed_files.push(ManagedFile {
                path: PathBuf::from(rel),
                strategy: MaterializeStrategy::Symlink,
                source: Some(source),
            });
        }
    }

    let state = ActivationState {
        schema_version: CURRENT_SCHEMA_VERSION,
        active_namespace: namespace_name.to_string(),
        project_root: project_root.to_path_buf(),
        managed_files,
        backed_up: Vec::new(),
    };
    fs.write(
        &project_root.join(".aenv/state.json"),
        state.to_json()?.as_bytes(),
    )?;
    Ok(state)
}

fn load_manifest<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    name: &str,
) -> Result<AenvManifest> {
    let path = layout.manifest_path(name);
    if !fs.exists(&path)? {
        return Err(AenvError::NamespaceNotFound(name.to_string()));
    }
    let bytes = fs.read(&path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", path.display())))?;
    AenvManifest::from_toml(text)
}

/// Compute the set of project-relative files an adapter manages for a given
/// manifest entry. Phase 1 just intersects the adapter's `files` with the
/// entry's `files`: a path managed by the adapter is materialized only if
/// the manifest also lists it.
fn adapter_files_for_entry(
    adapter: &crate::adapter::Adapter,
    entry: &crate::manifest::AdapterEntry,
) -> Vec<String> {
    let mut out = Vec::new();
    for f in &entry.files {
        if adapter.files.iter().any(|af| af == f || file_under_prefix(f, af)) {
            out.push(f.clone());
        }
    }
    out
}

/// Whether `file` is a relative path under the directory `prefix` (which
/// ends in `/`). Adapters declare directory prefixes like `.claude/` to
/// mean "everything under this path."
fn file_under_prefix(file: &str, prefix: &str) -> bool {
    if !prefix.ends_with('/') {
        return false;
    }
    file.starts_with(prefix)
}
```

- [ ] **Step 8.4: Wire into `lib.rs`**

```rust
pub mod activate;
pub mod adapter;
pub mod adapters_builtin;
pub mod atomicity;
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;
pub mod namespace;
pub mod project;
pub mod state;

pub use error::{AenvError, Result};
```

- [ ] **Step 8.5: Run tests (green)**

```bash
cargo test --package aenv-core --test activate
```

Expected: 4 tests pass.

- [ ] **Step 8.6: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add activate_namespace: symlink case for non-existent project files

Phase 1 activation skeleton. The simplest path: namespace ships file X,
adapter declares X, project doesn't have X -> symlink from project to
namespace, record state. Atomicity-probes .aenv/ before any
materialization (R-45 prerequisite). Unknown adapters in the manifest
abort with AdapterMissing (exit 11). Adapter-declared files the
namespace doesn't ship are skipped silently — adapters describe the
universe of paths managed, not what every namespace must ship.

Tasks 9 and 10 add the backup-on-displace and rollback paths.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Activation — backup-on-displace + byte-identical no-op

**Files:**
- Modify: `crates/aenv-core/src/activate.rs`
- Modify: `crates/aenv-core/tests/activate.rs`

**Purpose:** Cover R-45 (back up displaced files) and R-46 (byte-identical files become managed in place, not symlinked over). Backups land at `.aenv/backup/<ISO-timestamp>/<relative-path>`.

- [ ] **Step 9.1: Append failing tests**

Add to `crates/aenv-core/tests/activate.rs` (after the existing tests):

```rust
#[test]
fn backs_up_displaced_project_file() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"namespace disposition")]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), b"user-authored").unwrap();

    let state =
        activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
            .unwrap();

    // Project file is now a symlink.
    assert!(fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
    // Backup file holds the original contents.
    assert_eq!(state.backed_up.len(), 1);
    let backup = &state.backed_up[0];
    assert_eq!(backup.original_path, PathBuf::from("CLAUDE.md"));
    let backed_bytes = fs.read(&project.join(&backup.backup_path)).unwrap();
    assert_eq!(backed_bytes, b"user-authored");
}

#[test]
fn byte_identical_file_is_managed_in_place_not_symlinked() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let body: &[u8] = b"# CLAUDE.md\nshared content\n";
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", body)]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), body).unwrap();

    let state =
        activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
            .unwrap();

    // R-46: file matches namespace -> leave in place, do NOT symlink, mark managed.
    assert!(!fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
    assert_eq!(state.managed_files.len(), 1);
    assert_eq!(
        state.managed_files[0].strategy,
        MaterializeStrategy::Identical
    );
    assert!(state.backed_up.is_empty(), "no backup needed");
}

#[test]
fn aenv_managed_symlink_pointing_at_same_target_is_left_alone() {
    // Edge case: previous activation left a symlink pointing exactly where
    // we'd point now. No-op rather than backup + recreate.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"x")]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    // Pre-existing symlink to the same target.
    fs.symlink(
        &layout.namespace_dir("experiments").join("CLAUDE.md"),
        &project.join("CLAUDE.md"),
    )
    .unwrap();

    let state =
        activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
            .unwrap();

    // No backup; symlink stays.
    assert!(state.backed_up.is_empty());
    assert!(fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn stale_symlink_to_other_target_is_displaced() {
    // Regression: a project path that's a symlink to a non-aenv target (or
    // a stale aenv symlink whose target is gone) was previously
    // misclassified as Absent — exists() follows symlinks. The fix checks
    // symlink_metadata BEFORE exists, so the link itself surfaces as
    // Displaced and gets backed up rather than overwritten silently.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"new")]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    // Pre-existing symlink to a path that does NOT exist (stale).
    fs.symlink(
        &PathBuf::from("/elsewhere/CLAUDE.md"),
        &project.join("CLAUDE.md"),
    )
    .unwrap();

    let state =
        activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
            .unwrap();

    // Backed up the stale link; fresh symlink in place pointing at our source.
    assert_eq!(state.backed_up.len(), 1);
    assert_eq!(state.backed_up[0].original_path, PathBuf::from("CLAUDE.md"));
    assert_eq!(
        fs.read_link(&project.join("CLAUDE.md")).unwrap(),
        layout.namespace_dir("experiments").join("CLAUDE.md")
    );
}
```

- [ ] **Step 9.2: Run tests (red)**

```bash
cargo test --package aenv-core --test activate
```

Expected: the three new tests fail. Existing 4 still pass.

- [ ] **Step 9.3: Extend `activate_namespace` to handle displaced + identical cases**

Modify `crates/aenv-core/src/activate.rs` — replace the `for (adapter_name, entry) in &manifest.adapters` loop body:

```rust
    let timestamp = backup_timestamp();
    let backup_root = project_root.join(format!(".aenv/backup/{timestamp}"));
    let mut backed_up = Vec::new();
    let mut managed_files = Vec::new();

    for (adapter_name, entry) in &manifest.adapters {
        let adapter = adapters.get(adapter_name).expect("checked above");
        for rel in adapter_files_for_entry(adapter, entry) {
            let source = layout.namespace_dir(namespace_name).join(&rel);
            if !fs.exists(&source)? {
                continue;
            }
            let project_path = project_root.join(&rel);

            // What's currently at the project path?
            let action = classify_project_path(fs, &project_path, &source)?;
            match action {
                ProjectPathState::Absent => {
                    fs.symlink(&source, &project_path)?;
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
                ProjectPathState::AlreadyOurSymlink => {
                    // No-op; record as managed.
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
                ProjectPathState::ByteIdenticalRegular => {
                    // R-46: leave in place, mark managed.
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Identical,
                        source: None,
                    });
                }
                ProjectPathState::Displaced => {
                    let backup_rel = PathBuf::from(format!(".aenv/backup/{timestamp}")).join(&rel);
                    let backup_path = project_root.join(&backup_rel);
                    // Refuse to clobber an existing backup file at the
                    // target — protects R-61 against nanosecond-precision
                    // collisions and against stray backup contents.
                    if fs.exists(&backup_path)? {
                        return Err(AenvError::ActivationConflict(format!(
                            "backup path already exists: {}",
                            backup_path.display()
                        )));
                    }
                    // Ensure the backup directory exists.
                    if let Some(parent) = backup_path.parent() {
                        fs.create_dir_all(parent)?;
                    }
                    fs.rename(&project_path, &backup_path)?;
                    fs.symlink(&source, &project_path)?;
                    backed_up.push(BackedUpFile {
                        original_path: PathBuf::from(&rel),
                        backup_path: backup_rel,
                    });
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
            }
        }
    }
```

Add the supporting types and `classify_project_path` function:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectPathState {
    /// Nothing at the project path.
    Absent,
    /// Already an aenv-managed symlink pointing at our intended source.
    AlreadyOurSymlink,
    /// Regular file whose contents match the namespace's source.
    ByteIdenticalRegular,
    /// Something exists and differs — must back up.
    Displaced,
}

/// Decide what to do with the project path before materializing the source.
///
/// **Important:** we check `symlink_metadata` BEFORE `exists` because
/// `exists` follows symlinks. A stale aenv-managed symlink (target deleted)
/// would return `Ok(false)` from `exists` and get misclassified as Absent;
/// we'd then try to create a fresh symlink on top, fail with EEXIST on real
/// fs, and have no undo entry. Checking the link itself first closes this
/// hole. (Phase 0.5 P0 bug.)
fn classify_project_path<F: Filesystem>(
    fs: &F,
    project_path: &Path,
    source: &Path,
) -> Result<ProjectPathState> {
    // Inspect the path itself, not what it points to. NotFound means
    // nothing at the path; any other error propagates.
    let meta = match fs.symlink_metadata(project_path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProjectPathState::Absent);
        }
        Err(e) => return Err(AenvError::Io(e)),
    };
    if matches!(meta.kind, crate::fs::FileKind::Symlink) {
        let target = fs.read_link(project_path)?;
        if target == source {
            return Ok(ProjectPathState::AlreadyOurSymlink);
        }
        // Stale or other-target symlink: displace it. The backup will be
        // the link itself (rename moves the link, not the target). Reading
        // the backup later dereferences to whatever the link pointed at;
        // dangling-target case behaves as it did before activation.
        return Ok(ProjectPathState::Displaced);
    }
    // Regular file: compare bytes for the identical-case shortcut.
    if matches!(meta.kind, crate::fs::FileKind::File) {
        let project_bytes = fs.read(project_path)?;
        let source_bytes = fs.read(source)?;
        if project_bytes == source_bytes {
            return Ok(ProjectPathState::ByteIdenticalRegular);
        }
    }
    Ok(ProjectPathState::Displaced)
}

/// Filesystem-safe timestamp string for backup directory names.
///
/// Uses nanosecond precision so two activations within the same wall-clock
/// second don't collide. Same-nanosecond collisions are vanishingly rare;
/// if one ever happens we still avoid silent overwrite by checking for
/// directory existence at the caller (see the `Displaced` arm).
fn backup_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("epoch-{nanos}")
}
```

Also add the `BackedUpFile` import:

```rust
use crate::state::{
    ActivationState, BackedUpFile, ManagedFile, MaterializeStrategy, CURRENT_SCHEMA_VERSION,
};
```

And populate `backed_up` in the returned `ActivationState`:

```rust
    let state = ActivationState {
        schema_version: CURRENT_SCHEMA_VERSION,
        active_namespace: namespace_name.to_string(),
        project_root: project_root.to_path_buf(),
        managed_files,
        backed_up,
    };
```

- [ ] **Step 9.4: Run tests (green)**

```bash
cargo test --package aenv-core --test activate
```

Expected: all 8 activate tests pass (4 from Task 8 + 4 from Task 9 including the stale-symlink regression).

- [ ] **Step 9.5: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Extend activate: backup-on-displace + byte-identical + same-symlink

Cases R-45 / R-46 require:
- Project file exists and differs from namespace -> rename to
  .aenv/backup/<timestamp>/<relpath>, then symlink. Record both
  managed file and backup entry in state.
- Project file is byte-identical to namespace -> leave in place,
  mark as MaterializeStrategy::Identical, no symlink. No backup
  needed.
- Project file is already a symlink to exactly the same source ->
  no-op, record as managed.
- Project file is a stale or differently-targeted symlink -> back
  up the link itself, install fresh symlink. Regression test
  guards against the "symlink_metadata before exists" ordering bug
  reviewers caught in plan review.

classify_project_path checks symlink_metadata FIRST, falling back
to exists() only for non-symlink paths — `exists` follows symlinks
and would misclassify stale ones as Absent.

Backup directory naming uses epoch nanoseconds so two activations
in the same wall-clock second don't collide. The Displaced arm also
refuses to clobber any pre-existing file at the backup target, so
even a nanosecond collision surfaces as ActivationConflict instead
of silently destroying a prior backup (R-61).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Activation — rollback on partial failure

**Files:**
- Modify: `crates/aenv-core/src/activate.rs`
- Modify: `crates/aenv-core/tests/activate.rs`

**Purpose:** R-63 requires rolling back any partial materialization if activation fails mid-way. We track every operation we performed in an undo log and replay it in reverse on error.

- [ ] **Step 10.1: Append failing test using fail injection**

Add to `crates/aenv-core/tests/activate.rs`:

```rust
#[test]
fn rolls_back_when_state_write_fails_after_displacement() {
    // Setup: a project with a user-authored CLAUDE.md (which will be
    // displaced to backup during activation), then inject a write failure
    // on .aenv/state.json — the final write in perform_activation. The
    // failure fires after both the backup-rename and the symlink-create
    // have succeeded, so rollback must replay BOTH undo steps in reverse:
    // remove the new symlink, then rename the backup back into place.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"namespace")]);

    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), b"user-authored").unwrap();
    fs.fail_writes_to(&project.join(".aenv/state.json"));

    let err = activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
        .expect_err("must error");
    assert!(matches!(
        err,
        aenv_core::AenvError::Io(_) | aenv_core::AenvError::ActivationConflict(_)
    ));

    // Rollback invariants:
    // 1. No symlink left at the project path.
    assert!(
        !fs.is_symlink(&project.join("CLAUDE.md")).unwrap_or(false),
        "symlink should be rolled back"
    );
    // 2. Original content restored.
    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"user-authored");
    // 3. No state.json on disk.
    assert!(!fs.exists(&project.join(".aenv/state.json")).unwrap());
}
```

- [ ] **Step 10.2: Run test (red)**

```bash
cargo test --package aenv-core --test activate -- rolls_back
```

Expected: FAIL — rollback not yet implemented.

- [ ] **Step 10.3: Implement rollback via undo log**

Modify `crates/aenv-core/src/activate.rs`. Introduce an `UndoStep` enum that records every reversible operation, then build an `undo_log: Vec<UndoStep>` as we go. If anything fails, replay the log in reverse and return the original error.

Add at the top of the file:

```rust
enum UndoStep {
    /// Created a symlink at `link`; undo by removing it.
    RemoveSymlink { link: PathBuf },
    /// Backed up `original` to `backup`; undo by renaming `backup` -> `original`.
    RestoreBackup { original: PathBuf, backup: PathBuf },
}

fn undo<F: Filesystem>(fs: &F, log: Vec<UndoStep>) {
    // Replay in reverse; best-effort (we're already in an error path, so
    // we can't recursively bail on a failed undo step).
    for step in log.into_iter().rev() {
        match step {
            UndoStep::RemoveSymlink { link } => {
                let _ = fs.remove_file(&link);
            }
            UndoStep::RestoreBackup { original, backup } => {
                let _ = fs.rename(&backup, &original);
            }
        }
    }
}
```

Wrap the main loop and state-write with a helper that catches errors:

```rust
    // Build the materialization plan and execute it, tracking an undo log.
    let mut undo_log: Vec<UndoStep> = Vec::new();
    let result = perform_activation(
        fs,
        layout,
        adapters,
        project_root,
        namespace_name,
        &manifest,
        &mut undo_log,
    );
    match result {
        Ok(state) => Ok(state),
        Err(e) => {
            undo(fs, undo_log);
            Err(e)
        }
    }
}
```

(Replace the original loop body — extract it into `perform_activation`.)

Pull the loop into a helper:

```rust
fn perform_activation<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    namespace_name: &str,
    manifest: &AenvManifest,
    undo_log: &mut Vec<UndoStep>,
) -> Result<ActivationState> {
    let timestamp = backup_timestamp();
    let mut managed_files = Vec::new();
    let mut backed_up = Vec::new();

    for (adapter_name, entry) in &manifest.adapters {
        let adapter = adapters.get(adapter_name).expect("checked above");
        for rel in adapter_files_for_entry(adapter, entry) {
            let source = layout.namespace_dir(namespace_name).join(&rel);
            if !fs.exists(&source)? {
                continue;
            }
            let project_path = project_root.join(&rel);
            let action = classify_project_path(fs, &project_path, &source)?;
            match action {
                ProjectPathState::Absent => {
                    fs.symlink(&source, &project_path)?;
                    undo_log.push(UndoStep::RemoveSymlink {
                        link: project_path.clone(),
                    });
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
                ProjectPathState::AlreadyOurSymlink => {
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
                ProjectPathState::ByteIdenticalRegular => {
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Identical,
                        source: None,
                    });
                }
                ProjectPathState::Displaced => {
                    let backup_rel = PathBuf::from(format!(".aenv/backup/{timestamp}")).join(&rel);
                    let backup_path = project_root.join(&backup_rel);
                    if let Some(parent) = backup_path.parent() {
                        fs.create_dir_all(parent)?;
                    }
                    fs.rename(&project_path, &backup_path)?;
                    undo_log.push(UndoStep::RestoreBackup {
                        original: project_path.clone(),
                        backup: backup_path.clone(),
                    });
                    fs.symlink(&source, &project_path)?;
                    undo_log.push(UndoStep::RemoveSymlink {
                        link: project_path.clone(),
                    });
                    backed_up.push(BackedUpFile {
                        original_path: PathBuf::from(&rel),
                        backup_path: backup_rel,
                    });
                    managed_files.push(ManagedFile {
                        path: PathBuf::from(&rel),
                        strategy: MaterializeStrategy::Symlink,
                        source: Some(source.clone()),
                    });
                }
            }
        }
    }

    let state = ActivationState {
        schema_version: CURRENT_SCHEMA_VERSION,
        active_namespace: namespace_name.to_string(),
        project_root: project_root.to_path_buf(),
        managed_files,
        backed_up,
    };
    fs.write(
        &project_root.join(".aenv/state.json"),
        state.to_json()?.as_bytes(),
    )?;
    Ok(state)
}
```

- [ ] **Step 10.4: Run tests (green)**

```bash
cargo test --package aenv-core --test activate
```

Expected: all 8 activate tests pass.

- [ ] **Step 10.5: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add rollback on partial activation failure

R-63: if anything fails after we've started materializing, undo what
we did before bubbling the error. Implementation: every reversible
operation pushes an UndoStep onto a log; on error we replay the log
in reverse (best-effort — we can't recursively bail on a failed undo).

Two UndoStep variants for Phase 1: RemoveSymlink for symlinks we
created, RestoreBackup for files we renamed into the backup dir. The
displaced-file path pushes both, in the right order, so an error
between rename and symlink (or after symlink) leaves the project
exactly as we found it.

Tested via MockFilesystem::fail_writes_to injection on
.aenv/state.json — the final write — so the rollback path exercises
both undo variants in sequence.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Deactivation

**Files:**
- Create: `crates/aenv-core/src/deactivate.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/deactivate.rs`

**Purpose:** Reverse activation: remove every symlink we created, restore every backup, delete the state file. Files created by the user during activation (not in the state file) are left untouched (R-48).

- [ ] **Step 11.1: Write the failing test**

Create `crates/aenv-core/tests/deactivate.rs`:

```rust
//! Tests for deactivate_namespace.

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::deactivate::deactivate_namespace;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::create_namespace;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

fn registry_with_claude() -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    r.insert(Adapter {
        name: "claude-code".to_string(),
        files: vec!["CLAUDE.md".to_string()],
        merge_strategies: Default::default(),
    });
    r
}

fn setup_namespace(fs: &MockFilesystem, ns: &str, body: &[u8]) {
    let layout = layout();
    create_namespace(fs, &layout, ns).unwrap();
    fs.write(
        &layout.manifest_path(ns),
        format!("name = \"{ns}\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n")
            .as_bytes(),
    )
    .unwrap();
    fs.write(&layout.namespace_dir(ns).join("CLAUDE.md"), body).unwrap();
}

#[test]
fn deactivate_removes_symlink_and_state() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"disposition");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
        .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
    assert!(!fs.exists(&project.join(".aenv/state.json")).unwrap());
}

#[test]
fn deactivate_restores_backed_up_originals() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"namespace");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), b"original").unwrap();
    activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
        .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    let restored = fs.read(&project.join("CLAUDE.md")).unwrap();
    assert_eq!(restored, b"original");
    assert!(!fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn deactivate_leaves_unmanaged_files_alone() {
    // R-48: aenv removes only files it materialized.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"x");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
        .unwrap();

    // User creates a file during activation.
    fs.write(&project.join("README.md"), b"user file").unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    // Symlink removed; user file untouched.
    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
    assert_eq!(fs.read(&project.join("README.md")).unwrap(), b"user file");
}

#[test]
fn deactivate_errors_when_no_state_file() {
    // "No active state to deactivate" is distinct from "no .aenv pin":
    // a user can have a pin (project is associated with a namespace) but
    // have never activated. ActivationConflict (exit 13) is the right
    // variant; ProjectNotPinned (exit 20) is specifically about the
    // pin file itself missing.
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = deactivate_namespace(&fs, &project).expect_err("must error");
    assert!(matches!(err, aenv_core::AenvError::ActivationConflict(_)));
    assert_eq!(err.exit_code(), 13);
}

#[test]
fn deactivate_leaves_identical_file_in_place() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let body: &[u8] = b"shared";
    setup_namespace(&fs, "experiments", body);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), body).unwrap(); // identical
    activate_namespace(&fs, &layout, &registry_with_claude(), &project, "experiments")
        .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    // Identical-strategy file is the user's; it stays.
    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), body);
}
```

- [ ] **Step 11.2: Run test (red)**

```bash
cargo test --package aenv-core --test deactivate
```

Expected: FAIL — `aenv_core::deactivate` does not exist.

- [ ] **Step 11.3: Implement deactivate**

Create `crates/aenv-core/src/deactivate.rs`:

```rust
//! Deactivation: remove every file aenv materialized, restore backups,
//! delete state.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::state::{ActivationState, MaterializeStrategy};
use std::path::Path;

/// Deactivate the namespace currently active in `project_root`.
///
/// Reads `.aenv/state.json` to know what to undo. Files with strategy
/// `Symlink` or `Copy` are removed; the corresponding backed-up original
/// (if any) is renamed back into place. Files with strategy `Identical`
/// are left alone — they were the user's to begin with. After a
/// successful deactivation, `.aenv/state.json` is removed.
///
/// Missing state.json -> `ActivationConflict` (exit 13). A missing pin
/// file is a distinct condition (`ProjectNotPinned`, exit 20) — a user
/// can be pinned but not activated.
pub fn deactivate_namespace<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let state_path = project_root.join(".aenv/state.json");
    if !fs.exists(&state_path)? {
        return Err(AenvError::ActivationConflict(format!(
            "no active namespace in {}",
            project_root.display()
        )));
    }
    let bytes = fs.read(&state_path)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        AenvError::ManifestInvalid(format!("state.json: not utf-8: {e}"))
    })?;
    let state = ActivationState::from_json(text)?;

    // Remove materialized files first.
    for file in &state.managed_files {
        let project_path = project_root.join(&file.path);
        match file.strategy {
            MaterializeStrategy::Symlink | MaterializeStrategy::Copy => {
                // Best-effort: user may have removed it already.
                let _ = fs.remove_file(&project_path);
            }
            MaterializeStrategy::Identical => {
                // Leave in place: it's the user's file.
            }
            MaterializeStrategy::Merged => {
                // Phase 2; treat like Symlink for now.
                let _ = fs.remove_file(&project_path);
            }
        }
    }

    // Restore backups (rename backup -> original).
    for backup in &state.backed_up {
        let original = project_root.join(&backup.original_path);
        let backup_path = project_root.join(&backup.backup_path);
        // If something now occupies the original path, remove it first.
        if fs.exists(&original)? {
            let _ = fs.remove_file(&original);
        }
        fs.rename(&backup_path, &original)?;
    }

    // Remove the state file last — its presence is the signal that there's
    // anything to deactivate.
    fs.remove_file(&state_path)?;
    Ok(())
}
```

- [ ] **Step 11.4: Wire into `lib.rs`**

```rust
pub mod activate;
pub mod adapter;
pub mod adapters_builtin;
pub mod atomicity;
pub mod deactivate;
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;
pub mod namespace;
pub mod project;
pub mod state;

pub use error::{AenvError, Result};
```

- [ ] **Step 11.5: Run tests (green)**

```bash
cargo test --package aenv-core --test deactivate
```

Expected: 5 tests pass.

- [ ] **Step 11.6: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add deactivate_namespace

Mirror of activate. Reads .aenv/state.json, removes every Symlink /
Copy / Merged file we materialized, leaves Identical-strategy files
in place (they were the user's), restores every backup by renaming
backup -> original, and finally deletes state.json. R-48: aenv
removes only what aenv materialized — user-created files during
activation are untouched.

Missing state file -> ActivationConflict (exit 13). Distinct from
ProjectNotPinned (exit 20), which is specifically about a missing
.aenv pin file. The user can still call `aenv restore` (Task 12) to
recover backups from an earlier session even with no active state.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Restore (standalone)

**Files:**
- Create: `crates/aenv-core/src/restore.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/restore.rs`

**Purpose:** R-62 — restore the most recent backup set even when no namespace is active. Useful for "I deactivated but lost my original" recovery.

- [ ] **Step 12.1: Write the failing test**

Create `crates/aenv-core/tests/restore.rs`:

```rust
//! Tests for `restore_latest_backup`.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::restore::restore_latest_backup;
use aenv_core::AenvError;
use std::path::PathBuf;

#[test]
fn restores_latest_backup_set() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");

    // Two backup sets; latest by lex order wins (epoch timestamps sort lex).
    fs.write(
        &project.join(".aenv/backup/epoch-1000/CLAUDE.md"),
        b"older",
    )
    .unwrap();
    fs.write(
        &project.join(".aenv/backup/epoch-2000/CLAUDE.md"),
        b"newer",
    )
    .unwrap();
    fs.write(&project.join("CLAUDE.md"), b"current symlink target").unwrap();

    restore_latest_backup(&fs, &project).unwrap();

    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"newer");
}

#[test]
fn restores_multiple_files_in_one_set() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.write(
        &project.join(".aenv/backup/epoch-1000/CLAUDE.md"),
        b"a",
    )
    .unwrap();
    fs.write(
        &project.join(".aenv/backup/epoch-1000/.claude/foo.md"),
        b"b",
    )
    .unwrap();

    restore_latest_backup(&fs, &project).unwrap();

    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"a");
    assert_eq!(fs.read(&project.join(".claude/foo.md")).unwrap(), b"b");
}

#[test]
fn errors_when_no_backups_exist() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = restore_latest_backup(&fs, &project).expect_err("must error");
    assert!(matches!(err, AenvError::ActivationConflict(_)));
}

#[test]
fn errors_when_aenv_dir_missing() {
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = restore_latest_backup(&fs, &project).expect_err("must error");
    assert!(matches!(err, AenvError::ActivationConflict(_)));
}

#[test]
fn restore_is_idempotent_re_running_reproduces_state() {
    // The doc comment promises: "the backup directory is left intact so
    // the same backup set can be restored repeatedly." Lock that promise.
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.write(
        &project.join(".aenv/backup/epoch-1000/CLAUDE.md"),
        b"original",
    )
    .unwrap();

    restore_latest_backup(&fs, &project).unwrap();
    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"original");

    // User edits the restored file.
    fs.write(&project.join("CLAUDE.md"), b"edited").unwrap();

    // Second restore overwrites the edit with the backup contents again.
    restore_latest_backup(&fs, &project).unwrap();
    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), b"original");

    // Backup file is still there (not consumed).
    assert_eq!(
        fs.read(&project.join(".aenv/backup/epoch-1000/CLAUDE.md")).unwrap(),
        b"original"
    );
}
```

- [ ] **Step 12.2: Run test (red)**

```bash
cargo test --package aenv-core --test restore
```

Expected: FAIL — `aenv_core::restore` does not exist.

- [ ] **Step 12.3: Implement restore**

Create `crates/aenv-core/src/restore.rs`:

```rust
//! Restore the most recent backup set.
//!
//! R-62: `aenv restore` restores the latest backup even when no namespace
//! is active. Useful when a user manually removed a symlink or wants to
//! recover an original after a forced deactivation.
//!
//! Restore semantics are **copy**, not move — the backup directory is left
//! intact so the same backup set can be restored repeatedly. Note that
//! `aenv deactivate` uses *rename* (move) semantics on the backup,
//! consuming it; if the user deactivates first, the backup is gone and
//! restore will report no backups available. Restore is the recovery path
//! for "deactivation never happened" or "the backup is still there because
//! it wasn't the most recent activation's."

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use std::path::Path;

/// Restore the most recent backup set under `<project>/.aenv/backup/`.
/// Latest is determined by lex-order on the timestamp directory name
/// (matching how `backup_timestamp()` formats it).
pub fn restore_latest_backup<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let backup_root = project_root.join(".aenv/backup");
    if !fs.exists(&backup_root)? {
        return Err(AenvError::ActivationConflict(
            "no backups found under .aenv/backup/".to_string(),
        ));
    }
    let mut sets = fs.list_dir(&backup_root)?;
    sets.sort();
    let latest = sets.last().ok_or_else(|| {
        AenvError::ActivationConflict("no backup sets in .aenv/backup/".to_string())
    })?;

    // Walk the backup set, restoring every file with the correct
    // project-relative path.
    let prefix = latest.clone();
    let mut to_visit = vec![prefix.clone()];
    while let Some(dir) = to_visit.pop() {
        for entry in fs.list_dir(&dir)? {
            let meta = fs.symlink_metadata(&entry)?;
            if matches!(meta.kind, crate::fs::FileKind::Directory) {
                to_visit.push(entry);
                continue;
            }
            // Compute the project-relative path by stripping the timestamp prefix.
            let rel = entry
                .strip_prefix(&prefix)
                .map_err(|e| AenvError::ActivationConflict(format!("bad backup path: {e}")))?
                .to_path_buf();
            let target = project_root.join(&rel);
            // If the project path currently has something at it, drop it.
            if fs.exists(&target)? {
                let _ = fs.remove_file(&target);
            }
            // Copy bytes (rename across the backup dir would change the
            // backup set; copy keeps the backup intact for re-restore).
            let bytes = fs.read(&entry)?;
            fs.write(&target, &bytes)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 12.4: Wire into `lib.rs`**

```rust
pub mod activate;
pub mod adapter;
pub mod adapters_builtin;
pub mod atomicity;
pub mod deactivate;
pub mod error;
pub mod fs;
pub mod home;
pub mod manifest;
pub mod namespace;
pub mod project;
pub mod restore;
pub mod state;

pub use error::{AenvError, Result};
```

- [ ] **Step 12.5: Run tests (green)**

```bash
cargo test --package aenv-core --test restore
```

Expected: 5 tests pass.

- [ ] **Step 12.6: Lint, fmt, commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git add crates/aenv-core/
git commit -m "$(cat <<'EOF'
Add restore_latest_backup

R-62: independent of activation, recover the most recent backup set
under .aenv/backup/. Latest is determined by lex order on the
timestamp directory name (backup_timestamp uses epoch seconds, which
sort correctly).

Restore copies bytes into the project path; the backup directory is
left intact so re-running restore reproduces the same state. If
something currently occupies the project path, it's removed first.

No backups -> ActivationConflict (exit 13). Distinct from the
"already-active" conflict but using the same code; we'll refine the
exit-code surface in Phase 7 if it bites.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 13: CLI path resolution + `--project` plumbing

**Files:**
- Create: `crates/aenv-cli/src/paths.rs`
- Modify: `crates/aenv-cli/src/main.rs`

**Purpose:** Resolve `AENV_HOME` from env var (or default to `~/.aenv`) and `--project` from CLI flag (or ancestor walk). These are the only places that read env vars or `current_dir()`; everything below takes absolute paths.

- [ ] **Step 13.1: Implement path resolution**

Create `crates/aenv-cli/src/paths.rs`:

```rust
//! Path resolution for the CLI layer.
//!
//! `AENV_HOME` (env var, default `~/.aenv`) and `--project` (flag,
//! default ancestor-walk from cwd) are resolved here into absolute paths.
//! Library code below the CLI never reads env vars or `current_dir()`.

use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::project::find_project_root;
use std::path::PathBuf;

/// Resolve the registry root (`AENV_HOME`).
pub fn resolve_aenv_home() -> Result<PathBuf> {
    if let Ok(explicit) = std::env::var("AENV_HOME") {
        let p = PathBuf::from(explicit);
        if !p.is_absolute() {
            return Err(AenvError::ManifestInvalid(format!(
                "AENV_HOME must be absolute, got '{}'",
                p.display()
            )));
        }
        return Ok(p);
    }
    let home = std::env::var("HOME").map_err(|_| {
        AenvError::ManifestInvalid("HOME not set; cannot derive default AENV_HOME".to_string())
    })?;
    Ok(PathBuf::from(home).join(".aenv"))
}

/// Resolve the project root, given an optional `--project` override.
/// Walks ancestors from `cwd` looking for `.aenv` when no override.
pub fn resolve_project_root<F: Filesystem>(
    fs: &F,
    explicit: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.is_absolute() {
            return Err(AenvError::ManifestInvalid(format!(
                "--project must be absolute, got '{}'",
                p.display()
            )));
        }
        return Ok(p);
    }
    let cwd = std::env::current_dir().map_err(AenvError::Io)?;
    find_project_root(fs, &cwd)
}
```

- [ ] **Step 13.2: Wire into `main.rs`**

Modify `crates/aenv-cli/src/main.rs` to add the `paths` module:

```rust
//! `aenv` command-line entry point.

use clap::Parser;

mod paths;

/// Top-level CLI definition.
#[derive(Debug, Parser)]
#[command(
    name = "aenv",
    version,
    about = "Virtual environments for AI coding harness configs",
    long_about = None,
)]
struct Cli {
    // Subcommands added in Task 14.
}

fn main() {
    let _cli = Cli::parse();
}
```

- [ ] **Step 13.3: Verify build (no tests yet — paths.rs has no tests of its own; CLI-level e2e covers it in Task 16)**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: all clean.

- [ ] **Step 13.4: Commit**

```bash
git add crates/aenv-cli/
git commit -m "$(cat <<'EOF'
Add CLI path resolution (AENV_HOME, --project)

The CLI layer is the only place that reads env vars and current_dir(),
per engineering §6. resolve_aenv_home: AENV_HOME env var or default to
$HOME/.aenv; rejects non-absolute paths. resolve_project_root: --project
flag or ancestor walk via find_project_root.

End-to-end testing via the binary subprocess lands in Task 16; the
library-level tests in aenv-core/tests already cover the resolution
helpers (find_project_root in tests/project.rs).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 14: CLI subcommands — create, list, delete, use, adapter

**Files:**
- Create: `crates/aenv-cli/src/cmd/mod.rs`
- Create: `crates/aenv-cli/src/cmd/create.rs`
- Create: `crates/aenv-cli/src/cmd/list.rs`
- Create: `crates/aenv-cli/src/cmd/delete.rs`
- Create: `crates/aenv-cli/src/cmd/use_.rs`
- Create: `crates/aenv-cli/src/cmd/adapter.rs`
- Modify: `crates/aenv-cli/src/main.rs`
- Modify: `crates/aenv-cli/Cargo.toml` (re-add `thiserror`)

**Purpose:** Wire the namespace-registry-side commands into the CLI. These don't touch the project — only the registry.

- [ ] **Step 14.1: Re-add `thiserror` to `aenv-cli/Cargo.toml`**

```toml
[dependencies]
aenv-core = { path = "../aenv-core" }
clap = { workspace = true }
thiserror = { workspace = true }
```

(Drop the placeholder comment.)

- [ ] **Step 14.2: Create the cmd module aggregator**

Create `crates/aenv-cli/src/cmd/mod.rs`:

```rust
//! CLI subcommand handlers.
//!
//! Each handler takes a `Filesystem` reference and a context struct,
//! returning `aenv_core::Result<()>`. The handlers do printing on success.

pub mod adapter;
pub mod create;
pub mod delete;
pub mod list;
pub mod use_;
```

- [ ] **Step 14.3: Implement `aenv create`**

Create `crates/aenv-cli/src/cmd/create.rs`:

```rust
//! `aenv create <name>` — scaffold a new namespace.

use aenv_core::adapters_builtin;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::create_namespace;
use aenv_core::Result;

/// Create a new namespace, installing built-in adapters on first run.
pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, name: &str) -> Result<()> {
    adapters_builtin::install_builtins(fs, &layout.adapters_dir())?;
    create_namespace(fs, layout, name)?;
    println!(
        "Created namespace '{}' at {}",
        name,
        layout.namespace_dir(name).display()
    );
    Ok(())
}
```

- [ ] **Step 14.4: Implement `aenv list`**

Create `crates/aenv-cli/src/cmd/list.rs`:

```rust
//! `aenv list` — print every namespace in the registry.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::list_namespaces;
use aenv_core::Result;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout) -> Result<()> {
    let names = list_namespaces(fs, layout)?;
    if names.is_empty() {
        println!("No namespaces in registry at {}", layout.root().display());
        return Ok(());
    }
    println!("NAME");
    for name in names {
        println!("{name}");
    }
    Ok(())
}
```

- [ ] **Step 14.5: Implement `aenv delete`**

Create `crates/aenv-cli/src/cmd/delete.rs`:

```rust
//! `aenv delete <name>` — remove a namespace from the registry.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::namespace::delete_namespace;
use aenv_core::Result;

pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, name: &str) -> Result<()> {
    // PRD R-4 expects checking that the namespace isn't currently active in
    // any tracked project. Phase 1 lacks a project-tracking registry, so we
    // can't verify this. Warn loudly before destroying the namespace; a
    // proper safety check arrives with the tracking work later (likely
    // Phase 6, when the shell hook gives us a natural place to maintain a
    // registry of activated projects).
    eprintln!(
        "warning: cannot verify namespace '{name}' is unused; \
         Phase 1 lacks project-tracking. Delete is irreversible."
    );
    delete_namespace(fs, layout, name)?;
    println!("Deleted namespace '{name}'");
    Ok(())
}
```

- [ ] **Step 14.6: Implement `aenv use`**

Create `crates/aenv-cli/src/cmd/use_.rs`:

```rust
//! `aenv use <name>` — write `.aenv` pin at the project root.

use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::project::write_pin;
use aenv_core::{AenvError, Result};
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    project_root: &Path,
    name: &str,
) -> Result<()> {
    // Validate the namespace exists before writing the pin — otherwise the
    // user gets a confusing error later from `aenv activate` instead of
    // immediate feedback.
    if !fs.exists(&layout.manifest_path(name))? {
        return Err(AenvError::NamespaceNotFound(name.to_string()));
    }
    write_pin(fs, project_root, name)?;
    println!(
        "Pinned {} to namespace '{}'",
        project_root.display(),
        name
    );
    Ok(())
}
```

- [ ] **Step 14.7: Implement `aenv adapter add` and `aenv adapter list`**

Create `crates/aenv-cli/src/cmd/adapter.rs`:

```rust
//! `aenv adapter add <path>` / `aenv adapter list`.

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::{AenvError, Result};
use std::path::Path;

/// Validate that an adapter name is filesystem-safe: non-empty, no path
/// separators, no parent-directory traversal, no leading dot. Rejecting
/// these closes a path-traversal hole — a malicious `name = "../../etc/passwd"`
/// in an adapter TOML would otherwise let `run_add` write outside
/// `adapters_dir`.
fn validate_adapter_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(AenvError::ManifestInvalid(
            "adapter name must not be empty".to_string(),
        ));
    }
    if name.starts_with('.') {
        return Err(AenvError::ManifestInvalid(format!(
            "adapter name must not start with '.': {name:?}"
        )));
    }
    for ch in name.chars() {
        if ch == '/' || ch == '\\' || ch == '\0' {
            return Err(AenvError::ManifestInvalid(format!(
                "adapter name contains illegal character {ch:?}: {name:?}"
            )));
        }
    }
    if name == ".." || name.contains("..") {
        return Err(AenvError::ManifestInvalid(format!(
            "adapter name must not contain '..': {name:?}"
        )));
    }
    Ok(())
}

pub fn run_add<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    source: &Path,
) -> Result<()> {
    let bytes = fs.read(source)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", source.display()))
    })?;
    let adapter = Adapter::from_toml(text)?;
    validate_adapter_name(&adapter.name)?;
    fs.create_dir_all(&layout.adapters_dir())?;
    let target = layout.adapters_dir().join(format!("{}.toml", adapter.name));
    fs.write(&target, text.as_bytes())?;
    println!("Installed adapter '{}' at {}", adapter.name, target.display());
    Ok(())
}

pub fn run_list<F: Filesystem>(fs: &F, layout: &RegistryLayout) -> Result<()> {
    let reg = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    if reg.is_empty() {
        println!("No adapters installed at {}", layout.adapters_dir().display());
        return Ok(());
    }
    println!("ADAPTER         FILES");
    for (name, adapter) in reg.iter() {
        println!("{:<15} {}", name, adapter.files.join(", "));
    }
    Ok(())
}
```

- [ ] **Step 14.8: Wire all of it into `main.rs`**

Replace `crates/aenv-cli/src/main.rs`:

```rust
//! `aenv` command-line entry point.

use aenv_core::fs::RealFilesystem;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

mod cmd;
mod paths;

#[derive(Debug, Parser)]
#[command(
    name = "aenv",
    version,
    about = "Virtual environments for AI coding harness configs",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new namespace in the registry.
    Create {
        /// Name of the namespace.
        name: String,
    },
    /// List every namespace in the registry.
    List,
    /// Delete a namespace from the registry.
    Delete {
        /// Name of the namespace.
        name: String,
    },
    /// Pin the current project to a namespace by writing `.aenv`.
    Use {
        /// Name of the namespace.
        name: String,
        /// Project root override (defaults to ancestor walk from cwd).
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Adapter operations.
    Adapter {
        #[command(subcommand)]
        action: AdapterAction,
    },
}

#[derive(Debug, Subcommand)]
enum AdapterAction {
    /// Install an adapter from a TOML file.
    Add {
        /// Source file.
        path: PathBuf,
    },
    /// List installed adapters.
    List,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let fs = RealFilesystem;

    let result = (|| -> aenv_core::Result<()> {
        let layout = aenv_core::home::RegistryLayout::new(paths::resolve_aenv_home()?);
        match cli.command {
            Command::Create { name } => cmd::create::run(&fs, &layout, &name),
            Command::List => cmd::list::run(&fs, &layout),
            Command::Delete { name } => cmd::delete::run(&fs, &layout, &name),
            Command::Use { name, project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::use_::run(&fs, &layout, &project_root, &name)
            }
            Command::Adapter { action } => match action {
                AdapterAction::Add { path } => cmd::adapter::run_add(&fs, &layout, &path),
                AdapterAction::List => cmd::adapter::run_list(&fs, &layout),
            },
        }
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}
```

- [ ] **Step 14.9: Verify build, lint, fmt**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --workspace 2>&1 | tail -3
```

Expected: build clean; all 46+ existing tests still pass (no new tests this task — CLI e2e is Task 16).

- [ ] **Step 14.10: Spot-check the binary**

```bash
cargo run --quiet --package aenv-cli -- --help
```

Expected: clap prints the usage block with `create`, `list`, `delete`, `use`, and `adapter` subcommands.

- [ ] **Step 14.11: Commit**

```bash
git add crates/aenv-cli/ Cargo.lock
git commit -m "$(cat <<'EOF'
Wire registry-side CLI subcommands: create / list / delete / use / adapter

aenv-cli main.rs dispatches via clap subcommands to thin handlers under
src/cmd/. Each handler takes a Filesystem reference + RegistryLayout,
returning aenv_core::Result. main() maps errors to exit codes via
AenvError::exit_code() so the PRD R-82 contract is honored.

create installs built-in adapters on first run (idempotent). Use --project
to override ancestor-walk-from-cwd. adapter add/list manage the adapters
dir directly.

E2E coverage (binary as subprocess against real tempdir) lands in Task 16.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 15: CLI subcommands — activate, deactivate, restore, status

**Files:**
- Create: `crates/aenv-cli/src/cmd/activate.rs`
- Create: `crates/aenv-cli/src/cmd/deactivate.rs`
- Create: `crates/aenv-cli/src/cmd/restore.rs`
- Create: `crates/aenv-cli/src/cmd/status.rs`
- Modify: `crates/aenv-cli/src/cmd/mod.rs`
- Modify: `crates/aenv-cli/src/main.rs`

**Purpose:** Wire the project-side commands. These all take `--project` (or ancestor-walk).

- [ ] **Step 15.1: Implement activate**

Create `crates/aenv-cli/src/cmd/activate.rs`:

```rust
//! `aenv activate [<name>] [--project <path>]`.
//!
//! If `name` is omitted, read the project's `.aenv` pin.

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::project::read_pin;
use aenv_core::Result;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    project_root: &Path,
    namespace_name: Option<&str>,
) -> Result<()> {
    let name = match namespace_name {
        Some(n) => n.to_string(),
        None => read_pin(fs, project_root)?,
    };
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let state = activate_namespace(fs, layout, &adapters, project_root, &name)?;
    println!("Activated '{}' in {}", name, project_root.display());
    for file in &state.managed_files {
        println!("  + {} ({:?})", file.path.display(), file.strategy);
    }
    if !state.backed_up.is_empty() {
        println!("Backed up {} file(s):", state.backed_up.len());
        for backup in &state.backed_up {
            println!("  - {} -> {}", backup.original_path.display(), backup.backup_path.display());
        }
    }
    Ok(())
}
```

- [ ] **Step 15.2: Implement deactivate**

Create `crates/aenv-cli/src/cmd/deactivate.rs`:

```rust
//! `aenv deactivate [--project <path>]`.

use aenv_core::deactivate::deactivate_namespace;
use aenv_core::fs::Filesystem;
use aenv_core::Result;
use std::path::Path;

pub fn run<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    deactivate_namespace(fs, project_root)?;
    println!("Deactivated namespace in {}", project_root.display());
    Ok(())
}
```

- [ ] **Step 15.3: Implement restore**

Create `crates/aenv-cli/src/cmd/restore.rs`:

```rust
//! `aenv restore [--project <path>]`.

use aenv_core::fs::Filesystem;
use aenv_core::restore::restore_latest_backup;
use aenv_core::Result;
use std::path::Path;

pub fn run<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    restore_latest_backup(fs, project_root)?;
    println!("Restored most recent backup in {}", project_root.display());
    Ok(())
}
```

- [ ] **Step 15.4: Implement status**

Create `crates/aenv-cli/src/cmd/status.rs`:

```rust
//! `aenv status [--project <path>]`.

use aenv_core::fs::Filesystem;
use aenv_core::state::{ActivationState, MaterializeStrategy};
use aenv_core::Result;
use std::path::Path;

fn strategy_label(s: MaterializeStrategy) -> &'static str {
    match s {
        MaterializeStrategy::Symlink => "symlink",
        MaterializeStrategy::Copy => "copy",
        MaterializeStrategy::Identical => "identical",
        MaterializeStrategy::Merged => "merged",
    }
}

pub fn run<F: Filesystem>(fs: &F, project_root: &Path) -> Result<()> {
    let state_path = project_root.join(".aenv/state.json");
    if !fs.exists(&state_path)? {
        println!("No active namespace in {}", project_root.display());
        return Ok(());
    }
    let bytes = fs.read(&state_path)?;
    let text = String::from_utf8(bytes)
        .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("state.json: {e}")))?;
    let state = ActivationState::from_json(&text)?;
    println!("Active namespace: {}", state.active_namespace);
    println!("Project root: {}", state.project_root.display());
    println!("Managed files ({}):", state.managed_files.len());
    for file in &state.managed_files {
        println!("  {} ({})", file.path.display(), strategy_label(file.strategy));
    }
    if !state.backed_up.is_empty() {
        println!("Backed up ({}):", state.backed_up.len());
        for backup in &state.backed_up {
            println!("  {} -> {}", backup.original_path.display(), backup.backup_path.display());
        }
    }
    Ok(())
}
```

- [ ] **Step 15.5: Update `cmd/mod.rs`**

Replace `crates/aenv-cli/src/cmd/mod.rs`:

```rust
//! CLI subcommand handlers.

pub mod activate;
pub mod adapter;
pub mod create;
pub mod deactivate;
pub mod delete;
pub mod list;
pub mod restore;
pub mod status;
pub mod use_;
```

- [ ] **Step 15.6: Update `main.rs` to add the new subcommands**

Replace the `Command` enum and its dispatch in `crates/aenv-cli/src/main.rs`:

```rust
#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new namespace in the registry.
    Create {
        name: String,
    },
    /// List every namespace in the registry.
    List,
    /// Delete a namespace from the registry.
    Delete {
        name: String,
    },
    /// Pin the current project to a namespace by writing `.aenv`.
    Use {
        name: String,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Activate the pinned namespace (or a named one) in a project.
    Activate {
        /// Namespace name (defaults to the .aenv pin).
        name: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Deactivate the active namespace in a project.
    Deactivate {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Restore the most recent backup set in a project.
    Restore {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Show the active namespace and managed files in a project.
    Status {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Adapter operations.
    Adapter {
        #[command(subcommand)]
        action: AdapterAction,
    },
}
```

Update the dispatch `match cli.command { ... }` to handle the new commands:

```rust
            Command::Activate { name, project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::activate::run(&fs, &layout, &project_root, name.as_deref())
            }
            Command::Deactivate { project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::deactivate::run(&fs, &project_root)
            }
            Command::Restore { project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::restore::run(&fs, &project_root)
            }
            Command::Status { project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::status::run(&fs, &project_root)
            }
```

- [ ] **Step 15.7: Verify build, lint, fmt, test**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --workspace 2>&1 | tail -3
cargo run --quiet --package aenv-cli -- --help
```

Expected: build clean; tests pass; help lists all 9 subcommands.

- [ ] **Step 15.8: Commit**

```bash
git add crates/aenv-cli/
git commit -m "$(cat <<'EOF'
Wire project-side CLI subcommands: activate / deactivate / restore / status

Mirror of the registry-side wiring in Task 14. Each handler resolves
--project (or walks up to .aenv) and dispatches into aenv-core. activate
loads adapters from the registry, calls activate_namespace, prints a
human-readable summary of managed files + backups. status reads
.aenv/state.json and pretty-prints it.

End-to-end coverage against the real binary lands in Task 16.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 16: End-to-end CLI integration test

**Files:**
- Create: `crates/aenv-cli/tests/cli_e2e.rs`

**Purpose:** Drive the real binary as a subprocess against a real tempdir. This is the test that proves the CLI + library + filesystem stack works end-to-end. Per Phase 0.5's lesson ("the test that would have caught the contract gaps"), this is the load-bearing integration check before tagging.

- [ ] **Step 16.1: Write the e2e test**

Create `crates/aenv-cli/tests/cli_e2e.rs`:

```rust
//! End-to-end CLI integration test.
//!
//! Drives the built `aenv` binary as a subprocess against a real
//! `tempfile::tempdir`. Exercises the full happy path: create -> use ->
//! activate -> status -> deactivate -> restore.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: PathBuf,
    project: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
        // Canonicalize for macOS where /var is a symlink to /private/var —
        // tempdir().path() returns /var/folders/..., but `realpath` and
        // `read_link` return /private/var/folders/... . Use canonical paths
        // everywhere so equality assertions hold.
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        let project = std::fs::canonicalize(project_guard.path()).unwrap();
        Self {
            _aenv_home_guard: aenv_home_guard,
            _project_guard: project_guard,
            aenv_home,
            project,
        }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new(bin());
        c.env("AENV_HOME", &self.aenv_home);
        // Inherit HOME so the resolver doesn't blow up; we override AENV_HOME
        // anyway.
        c
    }

    fn aenv_home(&self) -> &Path {
        &self.aenv_home
    }

    fn project(&self) -> &Path {
        &self.project
    }
}

fn assert_success(out: std::process::Output, ctx: &str) {
    if !out.status.success() {
        panic!(
            "{ctx} failed: status={:?}, stdout={}, stderr={}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn happy_path_create_use_activate_status_deactivate() {
    let h = Harness::new();

    // 1. Create a namespace.
    let out = h.cmd().args(["create", "experiments"]).output().unwrap();
    assert_success(out, "create");

    // 2. Author a CLAUDE.md in the namespace.
    let ns_dir = h.aenv_home().join("envs/experiments");
    let ns_claude = ns_dir.join("CLAUDE.md");
    std::fs::write(&ns_claude, b"namespace disposition\n").unwrap();
    // Edit the manifest to register the claude-code adapter for CLAUDE.md.
    std::fs::write(
        ns_dir.join("aenv.toml"),
        b"name = \"experiments\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    // 3. Pin the project.
    let out = h
        .cmd()
        .args(["use", "experiments", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "use");
    assert_eq!(
        std::fs::read_to_string(h.project().join(".aenv")).unwrap().trim(),
        "experiments"
    );

    // 4. Activate.
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "activate");
    let project_claude = h.project().join("CLAUDE.md");
    let meta = std::fs::symlink_metadata(&project_claude).unwrap();
    assert!(meta.file_type().is_symlink(), "expected symlink");
    let target = std::fs::read_link(&project_claude).unwrap();
    assert_eq!(target, ns_claude);

    // 5. Status reports the active namespace.
    let out = h
        .cmd()
        .args(["status", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "status");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("experiments"));
    assert!(stdout.contains("CLAUDE.md"));

    // 6. Deactivate. The symlink goes away; no backup to restore (no original).
    let out = h
        .cmd()
        .args(["deactivate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "deactivate");
    assert!(!project_claude.exists());
    assert!(!h.project().join(".aenv/state.json").exists());
}

#[test]
fn backup_then_restore_round_trip() {
    let h = Harness::new();

    // Pre-populate project with a user CLAUDE.md.
    let project_claude = h.project().join("CLAUDE.md");
    std::fs::write(&project_claude, b"user-authored\n").unwrap();

    // Create + populate namespace.
    let out = h.cmd().args(["create", "experiments"]).output().unwrap();
    assert_success(out, "create");
    let ns_dir = h.aenv_home().join("envs/experiments");
    std::fs::write(ns_dir.join("CLAUDE.md"), b"namespace\n").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        b"name = \"experiments\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    // Activate -> user file backed up; symlink installed.
    let out = h
        .cmd()
        .args(["use", "experiments", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "use");
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "activate");
    assert!(std::fs::symlink_metadata(&project_claude).unwrap().file_type().is_symlink());

    // Deactivate -> backup is restored.
    let out = h
        .cmd()
        .args(["deactivate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "deactivate");
    let restored = std::fs::read_to_string(&project_claude).unwrap();
    assert_eq!(restored, "user-authored\n");
    // Symlink-bit gone.
    assert!(!std::fs::symlink_metadata(&project_claude).unwrap().file_type().is_symlink());
}

#[test]
fn list_after_create_shows_namespace() {
    let h = Harness::new();
    h.cmd().args(["create", "a"]).output().unwrap();
    h.cmd().args(["create", "b"]).output().unwrap();
    let out = h.cmd().args(["list"]).output().unwrap();
    assert_success(out.clone(), "list");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("a"));
    assert!(stdout.contains("b"));
}

#[test]
fn create_then_delete_round_trip() {
    let h = Harness::new();
    h.cmd().args(["create", "kill-me"]).output().unwrap();
    let out = h.cmd().args(["delete", "kill-me"]).output().unwrap();
    assert_success(out, "delete");
    assert!(!h.aenv_home().join("envs/kill-me").exists());
}

#[test]
fn activate_unknown_namespace_exits_ten() {
    let h = Harness::new();
    // Pin to a non-existent namespace.
    std::fs::write(h.project().join(".aenv"), b"nope\n").unwrap();
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(10));
}

#[test]
fn deactivate_without_active_state_exits_thirteen() {
    // Missing state.json -> ActivationConflict (exit 13), not
    // ProjectNotPinned (exit 20). The latter is reserved for missing
    // .aenv pin file specifically.
    let h = Harness::new();
    let out = h
        .cmd()
        .args(["deactivate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(13));
}

#[test]
fn activate_never_writes_outside_adapter_declared_paths() {
    // PRD R-60 invariant: aenv shall never modify a project file outside
    // the paths declared by active adapters. Activate, then enumerate
    // every regular file or symlink under the project root and assert
    // each one is either in the adapter's `files` set or under `.aenv/`.
    let h = Harness::new();

    // Setup: namespace ships CLAUDE.md only.
    let out = h.cmd().args(["create", "experiments"]).output().unwrap();
    assert_success(out, "create");
    let ns_dir = h.aenv_home().join("envs/experiments");
    std::fs::write(ns_dir.join("CLAUDE.md"), b"x").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        b"name = \"experiments\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let out = h
        .cmd()
        .args(["use", "experiments", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "use");
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(out, "activate");

    // Walk the project tree; every entry must be the .aenv pin file, under
    // .aenv/, or in the adapter's declared files set.
    let declared: std::collections::HashSet<PathBuf> =
        [PathBuf::from(".aenv"), PathBuf::from("CLAUDE.md")]
            .into_iter()
            .collect();
    walk_assert_only_declared(h.project(), h.project(), &declared);
}

fn walk_assert_only_declared(
    root: &Path,
    current: &Path,
    declared: &std::collections::HashSet<PathBuf>,
) {
    for entry in std::fs::read_dir(current).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap().to_path_buf();
        // Top-level only: .aenv/ subtree (state, backups) is aenv's own.
        if rel.starts_with(".aenv") {
            continue;
        }
        // Recurse into directories.
        let meta = std::fs::symlink_metadata(&path).unwrap();
        if meta.file_type().is_dir() {
            walk_assert_only_declared(root, &path, declared);
            continue;
        }
        // Files and symlinks must be in the declared set.
        assert!(
            declared.contains(&rel),
            "R-60 violation: project has un-declared file {rel:?}",
        );
    }
}
```

- [ ] **Step 16.2: Add `tempfile` to `aenv-cli/Cargo.toml` dev-dependencies**

Modify `crates/aenv-cli/Cargo.toml`:

```toml
[dependencies]
aenv-core = { path = "../aenv-core" }
clap = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 16.3: Run the e2e tests**

```bash
cargo test --package aenv-cli --test cli_e2e
```

Expected: 7 tests pass. Each test spawns the built `aenv` binary and exercises a complete happy or error path.

- [ ] **Step 16.4: Verify the full workspace**

```bash
cargo build --workspace
cargo test --workspace 2>&1 | grep -E "^test result:" | tee /tmp/aenv-phase1-test.log
! grep -E "test result: FAILED" /tmp/aenv-phase1-test.log
total=$(awk '/^test result: ok\./ { sum += $4 } END { print sum }' /tmp/aenv-phase1-test.log)
echo "total tests: $total"
test "$total" -ge 80
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
```

Expected: all clean; total tests ≥ 80 (was 46 at Phase 0.5; +many from Phase 1).

- [ ] **Step 16.5: Commit**

```bash
git add crates/aenv-cli/
git commit -m "$(cat <<'EOF'
Add end-to-end CLI integration tests

Drives the built aenv binary as a subprocess against real tempdir
projects + AENV_HOME. Exercises the full happy path (create -> use ->
activate -> status -> deactivate), the backup-restore round trip
(activate displaces a user file, deactivate restores it), and two
error paths that verify exit codes (10 namespace-not-found, 20
project-not-pinned).

This is the test class that would have caught any disconnect between
the library logic and the binary plumbing — exit code mapping,
--project flag wiring, adapters_dir resolution, real-filesystem
symlink semantics. The mock can't catch these; tempdir-driven
subprocess tests can.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 1 verification

- [ ] **Step V1: Full toolchain green**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
```

All five must exit 0.

- [ ] **Step V2: Binary runs and supports all 9 subcommands**

```bash
cargo run --quiet --package aenv-cli -- --version
cargo run --quiet --package aenv-cli -- --help
```

Expected: version prints; help lists `create`, `list`, `delete`, `use`, `activate`, `deactivate`, `restore`, `status`, `adapter`.

- [ ] **Step V3: Test counts**

```bash
cargo test --workspace 2>&1 | tee /tmp/aenv-phase1-test.log
grep -E "^test result:" /tmp/aenv-phase1-test.log
! grep -E "test result: FAILED" /tmp/aenv-phase1-test.log
total=$(awk '/^test result: ok\./ { sum += $4 } END { print sum }' /tmp/aenv-phase1-test.log)
echo "total tests: $total"
test "$total" -ge 80
```

Expected: all "ok"; no FAILED; total ≥ 80 (≥ 46 Phase 0.5 baseline + Phase 1 additions).

- [ ] **Step V4: Commit history is clean**

```bash
git log --oneline phase-0-complete..HEAD
```

Expected: 16 task commits + the Phase 0.5 commits, in clean order.

- [ ] **Step V5: Tag `phase-1-complete` locally**

```bash
git tag -a phase-1-complete -m "Phase 1 — single-namespace happy path complete

Workspace, manifest+adapter parsing, namespace registry ops, project
pinning, activation (symlink + backup-on-displace + identical no-op +
rollback), deactivation, restore. Single adapter: claude-code. CLI has
9 subcommands (create / list / delete / use / activate / deactivate /
restore / status / adapter add+list). End-to-end tested via subprocess
against real tempdir."
git tag -l
```

Push is deliberately a *separate user decision*, not automated by this
verification step. When ready:

```bash
git push origin main
git push origin phase-1-complete
rtk proxy gh run watch $(rtk proxy gh run list --limit 1 --json databaseId -q '.[0].databaseId') --exit-status
```

Expected: CI green.

---

## Self-review checklist

**1. Spec coverage.** Walk PRD Phase 1 requirements (R-1..R-5, R-29..R-32 partial, R-33..R-34, R-37..R-38, R-43..R-46, R-48..R-50 partial, R-60..R-63, R-78..R-79, R-82..R-83):

- R-1 registry directory under `AENV_HOME`: Task 1 (`RegistryLayout`) + Task 13 (`resolve_aenv_home`).
- R-2 `aenv create`: Tasks 4 + 14.
- R-3 `aenv list`: Tasks 4 + 14.
- R-4 `aenv delete`: Tasks 4 + 14 (with caveat — see Task 4 doc on tracked-project safety).
- R-5 reject duplicate create: Task 4 (test `create_rejects_duplicate`).
- R-29..R-32 adapter plugin + `adapter add` / `adapter list`: Task 3 + Task 14. R-31 validation: Task 14 (`validate_adapter_name`) rejects path-traversal patterns before write.
- R-30 partial — Phase 1 ships only the claude-code built-in (Task 3). Cursor, Aider, Cline, Continue, Windsurf, generic MCP are Phase 2 deliverables.
- R-33 `.aenv` pin file: Task 5.
- R-34 `aenv use`: Task 14.
- R-37 `aenv activate`: Tasks 8–10 + 15.
- R-38 `aenv deactivate`: Tasks 11 + 15.
- R-43 state file: Task 6, written at end of activation.
- R-44 symlink new files: Task 8.
- R-45 backup displaced files: Task 9.
- R-46 byte-identical = managed in place: Task 9.
- R-48 deactivate touches only what aenv materialized: Task 11.
- R-50 `aenv status` (text, single namespace): Task 15.
- R-60 never modify outside adapter-declared paths: enforced by limiting writes to adapter `files` + `.aenv/` only. Task 16 includes `activate_never_writes_outside_adapter_declared_paths`, an end-to-end walk that asserts the invariant.
- R-61 never delete backups except explicit cleanup: deactivate does NOT delete the backup directory.
- R-62 `aenv restore`: Task 12.
- R-63 rollback on partial activation failure: Task 10.
- R-78 `--project` accepted everywhere: Tasks 13–15.
- R-79 `aenv activate <name> --project <path>`: Task 15.
- R-82 exit codes: 10 (Task 4 delete + Task 16 e2e), 11 (Task 8), 12 (Task 2/3/5), 13 (Task 7), 20 (Task 5/16) — all covered.
- R-83 exit codes documented: deferred to Phase 7's `--help` work; recorded in commit message.

**2. Placeholder scan.** Each task has complete code and exact commands. No "TBD", "implement later", "appropriate error handling," or stub references. The one allowance: forward-compat fields in `AenvManifest` are accepted-but-unused — explicitly noted in Task 2's commit message.

**3. Type consistency.** Spot-check key signatures across tasks:

- `Filesystem::write(path, contents)` — `&self` everywhere, matches Phase 0.5 contract. ✓
- `Filesystem::exists(path)` returns `io::Result<bool>` — every caller `.unwrap()`s or `?`s. ✓
- `ActivationState { schema_version, active_namespace, project_root, managed_files, backed_up }` — same shape in Tasks 6, 8, 9, 10, 11, 12, 15. ✓
- `MaterializeStrategy { Symlink, Copy, Identical, Merged }` — consistent across tasks. ✓
- `RegistryLayout::manifest_path(name)` returns `<root>/envs/<name>/aenv.toml` — Task 1 definition matches Task 4 / Task 8 usage. ✓
- `AdapterRegistry::load_from_dir(fs, &dir)` — Task 3 def, Task 8 / Task 14 / Task 15 usage. ✓
- `find_project_root(fs, start)` — Task 5 def, Task 13 usage. ✓

**4. Risk hotspots — addressed in plan revisions:**

- **Stale symlink classification:** `classify_project_path` checks `symlink_metadata` before `exists` (Task 9). Regression test `stale_symlink_to_other_target_is_displaced`.
- **Same-second backup collision:** `backup_timestamp` uses nanosecond precision, and the `Displaced` arm refuses to overwrite an existing backup path (Task 9). R-61 stays intact.
- **Adapter name path-traversal:** `validate_adapter_name` (Task 14) rejects `..`, `/`, `\`, leading `.`, and empty names before writing.
- **`aenv use` validates namespace existence** before writing the pin (Task 14) so users get immediate feedback instead of a confused activation later.
- **macOS canonicalization in e2e:** the harness canonicalizes both `AENV_HOME` and the project root via `std::fs::canonicalize` so `/var` ↔ `/private/var` symlink resolution doesn't break equality assertions.
- **Deactivate error variant:** missing state.json → `ActivationConflict` (exit 13), not `ProjectNotPinned`. The e2e test (`deactivate_without_active_state_exits_thirteen`) locks the exit code.
- **R-60 invariant:** Task 16's `activate_never_writes_outside_adapter_declared_paths` walks the post-activation project tree and rejects any file outside the declared set.

**5. Known limits (acceptable for Phase 1; addressed later):**

- **`aenv delete` safety check (R-4):** Phase 1 lacks a tracked-projects registry, so we can't fully verify a namespace isn't currently active anywhere. The CLI handler warns loudly; a real safety net arrives with the shell hook (Phase 6).
- **`aenv activate` on unknown namespace** errors with exit 10 (correct for manual activation). PRD R-36's warn-and-continue branch is specifically for the shell-hook auto-activation case (Phase 6); manual `aenv activate` keeps the error.
- **Backup directory cleanup:** `.aenv/backup/` accumulates across activate/deactivate cycles. R-61 keeps backups until explicit cleanup; an `aenv backup cleanup` command lands in Phase 7 alongside the rest of the cleanup commands.
- **Rollback best-effort:** if both `remove_file` and `rename` undo steps fail, the project is left in a partial state. Acceptable for Phase 1.

Plan ready to execute.
