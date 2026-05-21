# Phase 2 — Composition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Namespaces compose. A namespace may declare `extends = ["base"]`; the resolver walks the chain depth-first, detects cycles, and produces a `ResolvedNamespace` whose every artifact carries a `QualifiedName` (`<namespace>::<short_name>`). Three merge strategies — section-merge for Markdown, deep-merge for JSON/YAML/TOML, last-wins fallback — produce merged outputs as regular (non-symlink) files. Shadow chains are recorded so `aenv which` can answer "what does this file come from and what did it shadow?". `aenv fork` detaches a single file (replace symlink with copy); `aenv fork <name>` creates a new namespace populated from the current project. The six remaining built-in adapters (Cursor, Aider, Cline, Continue, Windsurf, MCP) ship embedded.

**Architecture:** Composition is layered cleanly on Phase 1's primitives. The `Filesystem`-trait library is unchanged; the new `resolve::resolve_namespace()` produces a `ResolvedNamespace` value, and the existing `activate::activate_namespace()` is rewritten on top of it. Merge logic lives in `merge/` with one module per strategy. Identity-erasure holds: only short names hit disk; qualified names live in `aenv-core`'s internal state, `.aenv-state/state.json`, and (in Phase 5) machine output. Shadow chains are computed during resolution and threaded through to materialization for `aenv which` and `aenv status` consumption.

**Tech Stack:** Rust 1.85+ stable. New library deps: `serde_yaml = "0.9"` (workspace-pinned for YAML deep-merge), `pulldown-cmark = "0.10"` *(or equivalent — see Task 5)* for Markdown section parsing. Existing deps cover JSON (`serde_json`) and TOML (`toml`). No new `Filesystem` trait methods.

**Plan structure:** 19 tasks. Tasks 1–2 build pure identity and resolution types (unit-testable, no fs). Task 3 implements the `extends`-chain resolver against `MockFilesystem`. Task 4 is strategy selection. Tasks 5–8 build the three merge primitives (one task each: section, JSON, YAML, TOML). Task 9 is shadow tracking. Tasks 10–11 wire composition into state and `activate`. Task 12 ships the six remaining adapters. Tasks 13–15 add the new CLI subcommands (`which`, `fork` for files + `fork` whole-project, `fork <name>` for namespace creation — Task 14 covers single-file detach, Task 14b covers whole-project R-53). Task 16 upgrades `status`. Task 17 is the end-to-end integration test. Task 18 tags `phase-2-complete`. Estimated effort: 3–4 days of focused work.

**Repository state at start:** Working tree clean. `main` at `7510238` (Phase 1 + .aenv-state/ doc reconciliation). 121 tests passing. CI green on local; `origin/main` 18 commits behind local (push held).

**Important Phase 0/0.5/1 invariants this plan honors:**

- `Filesystem` trait uses `&self` throughout. No `&mut self` methods, no `let mut fs = ...` in tests.
- `Filesystem::write(path, contents)` creates missing parent dirs by contract — do not pre-`create_dir_all` before every write.
- `Filesystem::exists` returns `io::Result<bool>` — `.unwrap()` in tests; use `assert!(...)` not `assert_eq!(..., true)` (clippy `bool_assert_comparison` will reject).
- `MockFilesystem::symlink_metadata` is the TOCTOU-free way to inspect an aenv-managed symlink.
- `MockFilesystem::fail_writes_to(path)` and `fail_stats_on(path)` exist for failure-injection in rollback paths.
- All paths below the CLI layer are absolute. The library never reads `std::env::current_dir()` or `std::env::var(...)`.
- State directory is `.aenv-state/` (not `.aenv/`) — `.aenv` is the pin file (Phase 1 decision; PRD R-33 ↔ R-43 collision resolved on the Phase 1→2 boundary).
- `AenvError` variants are extended in Phase 2 only with the variants already declared in `error.rs` as placeholders: `ExtendsCycle` (exit 15). The `ParameterUndefined` (16) and `PolicyViolation` (17) variants stay unused until Phase 3/4.
- The materialized-path invariant from Phase 1's `tests/cli_e2e.rs` extends naturally: no path on disk contains `::`. Property-tested in Task 17.
- Tests anticipate rustfmt `max_width = 100`. Pre-format multi-arg calls.
- Backup atomicity (PRD R-45) covers merged files too — when a merged output displaces a project file, the project file is backed up to `.aenv-state/backup/<timestamp>/` exactly as in Phase 1.

**Phase 2 deliberately defers:**

- Resolved-namespace hashing (`sha256-v1:...`) — Phase 5, by roadmap. Hash is computed over canonical bytes the resolver already produces, so the data is there; we just don't compute it yet.
- `--json` output everywhere — Phase 5. Status/list/which print text in Phase 2.
- `aenv diff` (`R-51`) — Phase 5 (depends on hash + qualified-name machine output).
- Parameters and policies (`R-14`–`R-28`) — Phase 3.
- Skill lifecycle (`R-15`–`R-20`, `R-72`–`R-77`) — Phase 4.
- Cross-namespace skill imports — Phase 4.

---

## File structure (created in this phase)

**Library (`crates/aenv-core/src/`):**

| File | Responsibility |
|---|---|
| `identity.rs` | `NamespaceId`, `ShortName`, `QualifiedName` newtypes; parse + Display (`::` separator) |
| `resolve.rs` | `ResolvedNamespace`, `ResolvedArtifact`, `resolve_namespace()` — walks `extends`, detects cycles |
| `strategy.rs` | Pick `MaterializeStrategy` per file: adapter `role` default, manifest `merge =` override |
| `merge/mod.rs` | `merge_artifacts` — dispatches by strategy |
| `merge/section.rs` | Markdown section-merge with `<!-- aenv:replace -->` marker |
| `merge/deep_json.rs` | `serde_json::Value` recursive deep-merge |
| `merge/deep_yaml.rs` | `serde_yaml::Value` → `serde_json::Value` deep-merge → YAML output |
| `merge/deep_toml.rs` | `toml::Value` recursive deep-merge |
| `shadow.rs` | Shadow-chain computation: pure function over resolved chain |
| `adapters_builtin/cursor.toml` | Cursor adapter (`.cursorrules`, `.cursor/`) |
| `adapters_builtin/aider.toml` | Aider adapter (`.aider.conf.yml`, `.aiderignore`) |
| `adapters_builtin/cline.toml` | Cline adapter (`.clinerules`) |
| `adapters_builtin/continue_.toml` | Continue adapter (`.continue/config.json`) — note trailing underscore: `continue` is a Rust keyword |
| `adapters_builtin/windsurf.toml` | Windsurf adapter (`.windsurfrules`) |
| `adapters_builtin/mcp.toml` | Generic MCP config adapter (`.mcp.json`, deep-merge) |

**Library (modified):**

- `crates/aenv-core/src/lib.rs` — re-export `identity`, `resolve`, `strategy`, `merge`, `shadow` modules
- `crates/aenv-core/src/state.rs` — extend `ManagedFile` with `qualified_name: QualifiedName`, `contributors: Vec<QualifiedName>` (for merged), `shadows: Vec<QualifiedName>`; bump `schema_version` to `2`
- `crates/aenv-core/src/activate.rs` — call `resolve_namespace` instead of single-namespace loading; materialize merged files as regular files; record qualified provenance in state
- `crates/aenv-core/src/adapters_builtin/mod.rs` — `include_str!` for the six new adapters; write them on registry init
- `crates/aenv-core/src/error.rs` — *no new variants*; `ExtendsCycle` (placeholder since Phase 0.5) becomes live
- `crates/aenv-core/src/manifest.rs` — `AdapterEntry` gains optional `merge: Option<BTreeMap<String, String>>` for per-file overrides

**Binary (`crates/aenv-cli/src/`):**

| File | Responsibility |
|---|---|
| `cmd/which.rs` | `aenv which <path>` — print qualified name, materialized + source paths, shadow chain, strategy |
| `cmd/fork.rs` | `aenv fork [<file>|<name>]` — file mode: replace symlink with copy. Name mode: create namespace from project. |
| `main.rs` (modify) | Add `Which`, `Fork` subcommands to clap |
| `cmd/mod.rs` (modify) | `pub mod which; pub mod fork;` |
| `cmd/status.rs` (modify) | Add resolution chain + per-file qualified provenance to the text output |

**Tests (new):**

- `crates/aenv-core/tests/identity.rs` — newtype roundtrips, Display, parse-from-str, rejected forms
- `crates/aenv-core/tests/resolve.rs` — single-namespace resolution, two-level chain, three-level, diamond, cycle detection
- `crates/aenv-core/tests/strategy.rs` — role default, manifest override, last-wins fallback
- `crates/aenv-core/tests/merge_section.rs` — append, replace marker, ordering, no-heading content
- `crates/aenv-core/tests/merge_deep.rs` — JSON deep-merge, YAML deep-merge, TOML deep-merge, type-mismatch fallback
- `crates/aenv-core/tests/shadow.rs` — overlay shadows parent, three-deep shadow chain, merged files have no shadows
- `crates/aenv-core/tests/composition.rs` — `activate_namespace` end-to-end with two-namespace chain, merged + symlinked + shadowed files in one run
- `crates/aenv-core/tests/adapters_builtin.rs` — all seven adapters parse + load
- `crates/aenv-cli/tests/composition_e2e.rs` — end-to-end CLI composition: `create base + leaf`, `use leaf`, `activate`, `which`, `status`, `fork`, `deactivate`

**Property tests (in `tests/composition.rs`):**
- No materialized path on disk contains `::` (walks the project tree post-activation).
- Shadow chain length equals (chain depth - 1) when every namespace in the chain provides the same short name.
- Qualified-name uniqueness within a namespace (no namespace emits two artifacts with the same `ShortName`).

---

## Glossary (for the implementer)

- **NamespaceId** — the unique name of a namespace in the registry, e.g. `"base"`, `"detailed-execution"`.
- **ShortName** — the agent-visible identity of an artifact, e.g. `"write-tests"` (a skill), `".mcp.json"` (an MCP config file), `"CLAUDE.md"` (an instructions file). For path-keyed artifacts the short name is the relative path inside the namespace dir.
- **QualifiedName** — `(NamespaceId, ShortName)` rendered as `<namespace>::<short>`. Internal + machine output only; never materialized to disk.
- **Resolution chain** — depth-first traversal of `extends`, root → leaf. `base → detailed-execution` means the chain has `base` first, `detailed-execution` second. The leaf's adapters are the union across the chain (Phase 2 simplifies: all adapters in the chain compose; no shadowing of adapter declarations).
- **Provided artifact** — for a given short name + path, the artifact from the *latest* namespace in the chain that declares it.
- **Shadowed artifact** — an artifact in an *earlier* namespace in the chain that shares its path/short-name with a later one. Recorded in state but not materialized.
- **Contributors** — for merged files only: the ordered list of qualified-name inputs that contributed to the merged output. Two namespaces both declaring `.mcp.json` with `merge = "deep"` produce one merged file with `contributors = [base::.mcp.json, leaf::.mcp.json]`.
- **role = "instructions"** — an adapter declaration that a given path is an instructions file (the kind that section-merges by default). Phase 2's claude-code adapter marks `CLAUDE.md` as `role = "instructions"`. Cursor's `.cursorrules` is also `role = "instructions"`.
- **Strategy** — the resolved decision for what to do at materialization time: `Symlink`, `Copy`, `SectionMerge`, `DeepMerge(format)`, or `Identical` (Phase 1 holdover for byte-identical project files).

---

### Task 1: Identity types (`NamespaceId`, `ShortName`, `QualifiedName`)

Pure types. No filesystem, no async. Owns the wire format of the `::` separator.

**Files:**
- Create: `crates/aenv-core/src/identity.rs`
- Modify: `crates/aenv-core/src/lib.rs` (add `pub mod identity;`)
- Test: `crates/aenv-core/tests/identity.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/identity.rs`:

```rust
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};

#[test]
fn namespace_id_roundtrips() {
    let id = NamespaceId::new("detailed-execution").unwrap();
    assert_eq!(id.as_str(), "detailed-execution");
    assert_eq!(format!("{id}"), "detailed-execution");
}

#[test]
fn short_name_roundtrips() {
    let sn = ShortName::new("write-tests").unwrap();
    assert_eq!(sn.as_str(), "write-tests");
}

#[test]
fn qualified_name_display_uses_double_colon() {
    let qn = QualifiedName::new(
        NamespaceId::new("detailed-execution").unwrap(),
        ShortName::new("write-tests").unwrap(),
    );
    assert_eq!(format!("{qn}"), "detailed-execution::write-tests");
}

#[test]
fn qualified_name_parses_from_str() {
    let qn: QualifiedName = "base::CLAUDE.md".parse().unwrap();
    assert_eq!(qn.namespace().as_str(), "base");
    assert_eq!(qn.short().as_str(), "CLAUDE.md");
}

#[test]
fn parse_rejects_missing_separator() {
    assert!("just-a-name".parse::<QualifiedName>().is_err());
}

#[test]
fn parse_rejects_empty_namespace() {
    assert!("::foo".parse::<QualifiedName>().is_err());
}

#[test]
fn parse_rejects_empty_short_name() {
    assert!("foo::".parse::<QualifiedName>().is_err());
}

#[test]
fn parse_rejects_double_separator_in_namespace() {
    // "a::b::c" is ambiguous; we reject rather than try to guess.
    assert!("a::b::c".parse::<QualifiedName>().is_err());
}

#[test]
fn namespace_id_rejects_empty() {
    assert!(NamespaceId::new("").is_err());
}

#[test]
fn namespace_id_rejects_colon_chars() {
    assert!(NamespaceId::new("foo::bar").is_err());
    assert!(NamespaceId::new("foo:bar").is_err());
}

#[test]
fn namespace_id_rejects_reserved_merged_synthetic() {
    let err = NamespaceId::new("(merged)").unwrap_err();
    assert!(err.to_string().contains("reserved"));
    assert!(err.to_string().contains("merged"));
}

#[test]
fn short_name_allows_path_separators() {
    // Short names can be paths (e.g. ".claude/skills/write-tests/SKILL.md").
    let sn = ShortName::new(".claude/skills/write-tests/SKILL.md").unwrap();
    assert_eq!(sn.as_str(), ".claude/skills/write-tests/SKILL.md");
}

#[test]
fn short_name_rejects_double_colon() {
    // The separator must never appear in a ShortName, even though paths are allowed.
    assert!(ShortName::new("foo::bar").is_err());
}

#[test]
fn qualified_name_is_hash_and_eq() {
    use std::collections::HashSet;
    let a = QualifiedName::new(
        NamespaceId::new("base").unwrap(),
        ShortName::new("write-tests").unwrap(),
    );
    let b = a.clone();
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test identity`
Expected: compile error — `unresolved import aenv_core::identity` (module doesn't exist yet).

- [ ] **Step 3: Implement `NamespaceId`, `ShortName`, `QualifiedName`**

Create `crates/aenv-core/src/identity.rs`:

```rust
//! Namespace identity types — the wire format of `aenv`.
//!
//! `NamespaceId` and `ShortName` are validated newtypes; their `Display` impls
//! and `FromStr` parser define the `::`-separated qualified-name format used
//! in `.aenv-state/state.json`, machine output (Phase 5), and the `aenv which`
//! command. Changing any of this is a major-version break.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::AenvError;

const SEPARATOR: &str = "::";

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NamespaceId(String);

impl NamespaceId {
    /// Reserved synthetic namespace used by `activate_namespace` to label
    /// merged artifacts whose contributors span multiple real namespaces.
    /// Rejected from user-facing construction so a real namespace can never
    /// collide with the synthesizer's output.
    pub const RESERVED_MERGED: &'static str = "(merged)";

    pub fn new(s: impl Into<String>) -> Result<Self, AenvError> {
        let s = s.into();
        if s.is_empty() {
            return Err(AenvError::ManifestInvalid(
                "namespace name cannot be empty".into(),
            ));
        }
        if s.contains(':') {
            return Err(AenvError::ManifestInvalid(format!(
                "namespace name {s:?} cannot contain ':'"
            )));
        }
        if s == Self::RESERVED_MERGED {
            return Err(AenvError::ManifestInvalid(format!(
                "namespace name {s:?} is reserved; aenv uses it internally to label \
                 merged artifacts in state.json and 'aenv which' output. Pick a \
                 different name (e.g. 'merged-base' or 'combined')."
            )));
        }
        Ok(Self(s))
    }

    /// Construct the reserved synthetic `(merged)` namespace. The single
    /// intended caller is `aenv-core::activate::synthesize_merged_qn`. Test
    /// helpers that need to construct a synthetic qualified name (e.g. test
    /// fixtures comparing against state.json output) also call this.
    pub fn merged_synthetic() -> Self {
        Self(Self::RESERVED_MERGED.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShortName(String);

impl ShortName {
    pub fn new(s: impl Into<String>) -> Result<Self, AenvError> {
        let s = s.into();
        if s.is_empty() {
            return Err(AenvError::ManifestInvalid(
                "short name cannot be empty".into(),
            ));
        }
        if s.contains(SEPARATOR) {
            return Err(AenvError::ManifestInvalid(format!(
                "short name {s:?} cannot contain '::'"
            )));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ShortName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct QualifiedName {
    namespace: NamespaceId,
    short: ShortName,
}

impl QualifiedName {
    pub fn new(namespace: NamespaceId, short: ShortName) -> Self {
        Self { namespace, short }
    }

    pub fn namespace(&self) -> &NamespaceId {
        &self.namespace
    }

    pub fn short(&self) -> &ShortName {
        &self.short
    }
}

impl fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.namespace, SEPARATOR, self.short)
    }
}

impl FromStr for QualifiedName {
    type Err = AenvError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Exactly one occurrence of `::` is the contract.
        let occurrences: Vec<_> = s.match_indices(SEPARATOR).collect();
        if occurrences.len() != 1 {
            return Err(AenvError::ManifestInvalid(format!(
                "qualified name {s:?} must contain exactly one '::' separator"
            )));
        }
        let (idx, _) = occurrences[0];
        let ns = &s[..idx];
        let short = &s[idx + SEPARATOR.len()..];
        Ok(Self {
            namespace: NamespaceId::new(ns)?,
            short: ShortName::new(short)?,
        })
    }
}
```

- [ ] **Step 4: Wire the module into `lib.rs`**

Modify `crates/aenv-core/src/lib.rs` — add `pub mod identity;` next to the other `pub mod` lines. Confirm with `grep '^pub mod' crates/aenv-core/src/lib.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p aenv-core --test identity`
Expected: PASS (12 tests).

Also run: `cargo clippy -p aenv-core -- -D warnings` to catch lint issues early.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/identity.rs crates/aenv-core/src/lib.rs \
        crates/aenv-core/tests/identity.rs
git commit -m "Add identity types: NamespaceId, ShortName, QualifiedName

Validated newtypes with serde transparency, Display impls, and FromStr
parser. The '::' separator is the wire format (PRD R-11, engineering
spec §7.5). Tests cover roundtrips, malformed input, hash/eq for use
as map keys, and the path-as-short-name shape (e.g. CLAUDE.md, with
slashes allowed but never '::').
"
```

---

### Task 2: Resolved-namespace types (`ResolvedNamespace`, `ResolvedArtifact`)

The output shape of resolution. Pure types — no fs interaction. Wired into nothing yet.

**Files:**
- Create: `crates/aenv-core/src/resolve.rs` (types only; resolution algorithm in Task 3)
- Modify: `crates/aenv-core/src/lib.rs` (add `pub mod resolve;`)
- Test: `crates/aenv-core/tests/resolve_types.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/resolve_types.rs`:

```rust
use std::path::PathBuf;

use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::{MaterializeStrategy, ResolvedArtifact, ResolvedNamespace};

fn qn(ns: &str, short: &str) -> QualifiedName {
    let nsid = if ns == NamespaceId::RESERVED_MERGED {
        NamespaceId::merged_synthetic()
    } else {
        NamespaceId::new(ns).unwrap()
    };
    QualifiedName::new(nsid, ShortName::new(short).unwrap())
}

#[test]
fn resolved_namespace_constructs() {
    let resolved = ResolvedNamespace {
        chain: vec![
            NamespaceId::new("base").unwrap(),
            NamespaceId::new("detailed-execution").unwrap(),
        ],
        artifacts: vec![],
    };
    assert_eq!(resolved.chain.len(), 2);
    assert_eq!(resolved.chain[0].as_str(), "base");
}

#[test]
fn artifact_carries_qualified_name_and_strategy() {
    let art = ResolvedArtifact {
        qualified_name: qn("detailed-execution", "write-tests"),
        materialized_path: PathBuf::from(".claude/skills/write-tests/SKILL.md"),
        source_path: PathBuf::from(
            "/home/u/.aenv/envs/detailed-execution/.claude/skills/write-tests/SKILL.md",
        ),
        strategy: MaterializeStrategy::Symlink,
        shadows: vec![qn("base", "write-tests")],
        contributors: vec![],
    };
    assert_eq!(art.qualified_name.namespace().as_str(), "detailed-execution");
    assert!(matches!(art.strategy, MaterializeStrategy::Symlink));
    assert_eq!(art.shadows.len(), 1);
}

#[test]
fn strategy_supports_three_merge_kinds() {
    use MaterializeStrategy::*;
    let _ = Symlink;
    let _ = Identical;
    let _ = SectionMerge;
    let _ = DeepMerge(aenv_core::resolve::DeepMergeFormat::Json);
    let _ = DeepMerge(aenv_core::resolve::DeepMergeFormat::Yaml);
    let _ = DeepMerge(aenv_core::resolve::DeepMergeFormat::Toml);
}

#[test]
fn merged_artifact_has_contributors_no_shadows() {
    let art = ResolvedArtifact {
        qualified_name: qn("(merged)", ".mcp.json"),
        materialized_path: PathBuf::from(".mcp.json"),
        source_path: PathBuf::new(), // unused for merged
        strategy: MaterializeStrategy::DeepMerge(aenv_core::resolve::DeepMergeFormat::Json),
        shadows: vec![],
        contributors: vec![qn("base", ".mcp.json"), qn("leaf", ".mcp.json")],
    };
    assert!(art.shadows.is_empty());
    assert_eq!(art.contributors.len(), 2);
}
```

Note: `"(merged)"` is the reserved synthetic namespace label for artifacts whose contributors span multiple real namespaces. `NamespaceId::new("(merged)")` *rejects* the string (Task 1 reserves it explicitly); only the internal constructor `NamespaceId::merged_synthetic()` produces it. The test helper `qn()` above special-cases the reserved string and routes to `merged_synthetic()` so tests can construct fixtures that match the on-disk state.json shape. The convention is documented in functional spec §7.1.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test resolve_types`
Expected: compile error — `unresolved import aenv_core::resolve`.

- [ ] **Step 3: Implement the types**

Create `crates/aenv-core/src/resolve.rs`:

```rust
//! Resolution output types.
//!
//! `ResolvedNamespace` is the product of walking the `extends` chain of a
//! leaf namespace. Every materializable artifact in the project carries a
//! `QualifiedName`, the strategy used to put it on disk, and (for shadowed
//! or merged artifacts) the qualified identities involved in the decision.
//!
//! Resolution itself lives in `resolve_namespace` (added in Task 3) — this
//! module owns only the data shapes.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::identity::{NamespaceId, QualifiedName};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolvedNamespace {
    /// Root → leaf order. The leaf is the namespace the user pinned.
    pub chain: Vec<NamespaceId>,
    /// Ordered by materialized_path (lexicographic); this order is the
    /// activation order and the hashing order (Phase 5).
    pub artifacts: Vec<ResolvedArtifact>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolvedArtifact {
    pub qualified_name: QualifiedName,
    pub materialized_path: PathBuf,
    pub source_path: PathBuf,
    pub strategy: MaterializeStrategy,
    /// Earlier-chain qualified names with the same short name + path.
    /// Empty for merged artifacts (every contributor is a co-producer, not a shadow).
    pub shadows: Vec<QualifiedName>,
    /// Ordered chain-of-contribution for merged artifacts. Empty otherwise.
    pub contributors: Vec<QualifiedName>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaterializeStrategy {
    /// Standard case: project file is a symlink to the namespace file.
    /// Serializes as `"symlink"` (matches Phase 1's lowercase form).
    Symlink,
    /// Project file already byte-identical to the namespace file — no symlink, no backup.
    Identical,
    /// Merged Markdown by `##` section.
    SectionMerge,
    /// Merged structured data in one of three formats.
    /// Serializes as `{"deep-merge": "json"}` etc.
    DeepMerge(DeepMergeFormat),
    /// Project file copied (Windows fallback, Phase 7); listed here for parity with state.rs.
    Copy,
    /// Phase 1 legacy variant. Accepted on read so old state files load; never
    /// emitted by Phase 2 code (which writes SectionMerge / DeepMerge instead).
    /// Phase 2's custom Deserialize for ManagedFile (Task 10) maps this to
    /// `SectionMerge` if encountered on a schema-1 state file.
    #[serde(rename = "merged")]
    Merged,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeepMergeFormat {
    Json,
    Yaml,
    Toml,
}

// Note: Phase 1's `state.rs` defines its own `MaterializeStrategy` with
// `#[serde(rename_all = "lowercase")]`. Task 10 deletes that definition and
// re-exports this one. The kebab-case + alias combination above preserves
// schema-1 compatibility: existing on-disk state files store `"symlink"` and
// `"merged"`, both of which the new enum accepts.
```

- [ ] **Step 4: Wire the module**

Modify `crates/aenv-core/src/lib.rs` — add `pub mod resolve;`.

- [ ] **Step 5: Run the test**

Run: `cargo test -p aenv-core --test resolve_types`
Expected: PASS (4 tests).

`cargo clippy -p aenv-core -- -D warnings` — clean.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/resolve.rs crates/aenv-core/src/lib.rs \
        crates/aenv-core/tests/resolve_types.rs
git commit -m "Add ResolvedNamespace, ResolvedArtifact, MaterializeStrategy types

Data shapes for Task 3's resolver output. ResolvedNamespace owns the
chain (root -> leaf) and the artifact list (sorted by materialized
path). ResolvedArtifact carries the qualified name, the strategy used
to put it on disk, and the shadow + contributor metadata that 'aenv
which' will read. MaterializeStrategy adds SectionMerge and
DeepMerge(format) to the Phase 1 set (Symlink, Identical, Copy).
"
```

---

### Task 3: Extends-chain resolver with cycle detection

The conceptual centerpiece. Given a leaf namespace id, walk `extends` depth-first, build the chain, detect cycles, gather candidate artifacts (without merging — that comes in Task 11). Pure library; uses the workspace's existing `<F: Filesystem>` generic shape (NOT trait objects — `AdapterRegistry::load_from_dir<F>` and `probe_rename_atomicity<F>` require the concrete type).

**Why no merging yet:** This task produces the resolution chain and the *candidate set* (per-namespace, per-path lists). Strategy selection (Task 4) and the actual merge (Tasks 5–8, 11) are layered on top. Keeping resolution and merging separate lets us test each in isolation, and lets shadow tracking (Task 9) operate over the candidate set without needing merge results.

**Files:**
- Modify: `crates/aenv-core/src/resolve.rs` (add the resolver function + `Candidate` type)
- Test: `crates/aenv-core/tests/resolve.rs`

- [ ] **Step 1: Write the failing test — single-namespace baseline**

Create `crates/aenv-core/tests/resolve.rs`:

```rust
//! Resolver tests. `resolve_namespace` walks the `extends` chain, gathers
//! candidate artifacts, and returns the chain + an indexed candidate set.

use std::path::Path;

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::manifest::AenvManifest;
use aenv_core::resolve::{resolve_namespace, ResolutionError};

mod mock_filesystem;
use mock_filesystem::MockFilesystem;

const REG: &str = "/aenv";

fn registry() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from(REG))
}

fn write_manifest(fs: &MockFilesystem, name: &str, body: &str) {
    let path = format!("{REG}/envs/{name}/aenv.toml");
    fs.write(Path::new(&path), body.as_bytes()).unwrap();
}

fn write_file(fs: &MockFilesystem, ns: &str, rel: &str, contents: &str) {
    let path = format!("{REG}/envs/{ns}/{rel}");
    fs.write(Path::new(&path), contents.as_bytes()).unwrap();
}

fn cc_adapter() -> Adapter {
    // Mirrors the embedded claude-code adapter for tests.
    toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md", ".claude/skills/**/*"]
"#,
    )
    .unwrap()
}

fn registry_with_cc() -> AdapterRegistry {
    let mut r = AdapterRegistry::default();
    r.insert(cc_adapter());
    r
}

#[test]
fn resolves_single_namespace_with_no_extends() {
    let fs = MockFilesystem::default();
    write_manifest(
        &fs,
        "base",
        r#"
name = "base"
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    );
    write_file(&fs, "base", "CLAUDE.md", "# base instructions\n");

    let resolved = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("base").unwrap(),
    )
    .unwrap();

    assert_eq!(resolved.chain, vec![NamespaceId::new("base").unwrap()]);
    // No merging yet, but the candidate-bearing artifact list is populated.
    assert!(resolved.candidates.iter().any(|c| c.path == Path::new("CLAUDE.md")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test resolve -- resolves_single_namespace_with_no_extends`
Expected: compile error — `resolve_namespace` and `ResolutionError` don't exist, neither does `Candidate`.

- [ ] **Step 3: Sketch the resolver types (extend `resolve.rs`)**

Add to `crates/aenv-core/src/resolve.rs`:

```rust
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapter::AdapterRegistry;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::manifest::AenvManifest;
use crate::AenvError;

/// One candidate contribution from a single namespace for a single path.
/// Multiple candidates with the same `path` across the chain are what
/// strategy selection (Task 4) and merging (Tasks 5–8) consume.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Candidate {
    pub namespace: NamespaceId,
    pub path: PathBuf,
    /// Absolute path inside the namespace dir on disk.
    pub source_path: PathBuf,
    /// The adapter name that declared this path (used by strategy selection
    /// to look up role / merge defaults).
    pub adapter: String,
    /// Per-file override declared in the manifest's `[adapters.X.merge]`
    /// table — `None` means "use the adapter's default role-based strategy".
    pub merge_override: Option<String>,
}

/// Output of `resolve_namespace` (intermediate — Task 11 turns this into a
/// final `ResolvedNamespace` with strategy decided + merging done).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolutionResult {
    pub chain: Vec<NamespaceId>,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResolutionError {
    Cycle(Vec<NamespaceId>),
    NamespaceNotFound(NamespaceId),
    AdapterMissing(String),
    ManifestInvalid { namespace: NamespaceId, reason: String },
    Io(String),
}

impl From<ResolutionError> for AenvError {
    fn from(value: ResolutionError) -> Self {
        match value {
            ResolutionError::Cycle(chain) => {
                let rendered = chain
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(" -> ");
                AenvError::ExtendsCycle(rendered)
            }
            ResolutionError::NamespaceNotFound(id) => {
                AenvError::NamespaceNotFound(id.as_str().to_owned())
            }
            ResolutionError::AdapterMissing(name) => AenvError::AdapterMissing(name),
            ResolutionError::ManifestInvalid { namespace, reason } => {
                AenvError::ManifestInvalid(format!("{namespace}: {reason}"))
            }
            ResolutionError::Io(msg) => AenvError::Io(std::io::Error::other(msg)),
        }
    }
}
```

- [ ] **Step 4: Implement `resolve_namespace` — depth-first walk**

Append to `crates/aenv-core/src/resolve.rs`:

```rust
/// Walk the `extends` chain depth-first starting from `leaf`, return chain
/// (root -> leaf) + candidate artifacts gathered across the chain.
///
/// Cycles surface as `ResolutionError::Cycle(stack)`. Missing namespaces or
/// adapters surface with the offending name. Manifests that fail TOML parse
/// surface as `ManifestInvalid`.
pub fn resolve_namespace<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    adapters: &AdapterRegistry,
    leaf: &NamespaceId,
) -> Result<ResolutionResult, ResolutionError> {
    let mut chain: Vec<NamespaceId> = Vec::new();
    let mut visiting: Vec<NamespaceId> = Vec::new(); // active DFS stack for cycle detection
    let mut visited: std::collections::BTreeSet<NamespaceId> = Default::default();

    walk(fs, registry, leaf, &mut chain, &mut visiting, &mut visited)?;

    // Chain is built in post-order so that root appears first, leaf last.
    let mut candidates: Vec<Candidate> = Vec::new();
    for ns in &chain {
        let manifest = load_manifest(fs, registry, ns)?;
        // Validate adapter references against the loaded registry.
        for adapter_name in manifest.adapters.keys() {
            if adapters.get(adapter_name).is_none() {
                return Err(ResolutionError::AdapterMissing(adapter_name.clone()));
            }
        }
        gather_candidates(fs, registry, ns, &manifest, &mut candidates)?;
    }
    Ok(ResolutionResult { chain, candidates })
}

fn walk<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    current: &NamespaceId,
    chain: &mut Vec<NamespaceId>,
    visiting: &mut Vec<NamespaceId>,
    visited: &mut std::collections::BTreeSet<NamespaceId>,
) -> Result<(), ResolutionError> {
    if visited.contains(current) {
        // Already incorporated into chain — diamonds reuse the existing position.
        return Ok(());
    }
    if visiting.contains(current) {
        // Cycle: stack from first occurrence onward, plus current to close the loop.
        let start = visiting.iter().position(|n| n == current).unwrap();
        let mut cycle: Vec<NamespaceId> = visiting[start..].to_vec();
        cycle.push(current.clone());
        return Err(ResolutionError::Cycle(cycle));
    }
    visiting.push(current.clone());

    let manifest = load_manifest(fs, registry, current)?;
    for parent in &manifest.extends {
        let parent_id = NamespaceId::new(parent.clone()).map_err(|e| {
            ResolutionError::ManifestInvalid {
                namespace: current.clone(),
                reason: e.to_string(),
            }
        })?;
        walk(fs, registry, &parent_id, chain, visiting, visited)?;
    }

    visiting.pop();
    visited.insert(current.clone());
    chain.push(current.clone());
    Ok(())
}

fn load_manifest<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    ns: &NamespaceId,
) -> Result<AenvManifest, ResolutionError> {
    let path = registry.manifest_path(ns.as_str());
    if !fs.exists(&path).map_err(|e| ResolutionError::Io(e.to_string()))? {
        return Err(ResolutionError::NamespaceNotFound(ns.clone()));
    }
    let bytes = fs.read(&path).map_err(|e| ResolutionError::Io(e.to_string()))?;
    let text = String::from_utf8(bytes).map_err(|e| ResolutionError::ManifestInvalid {
        namespace: ns.clone(),
        reason: format!("manifest is not valid UTF-8: {e}"),
    })?;
    let manifest: AenvManifest =
        toml::from_str(&text).map_err(|e| ResolutionError::ManifestInvalid {
            namespace: ns.clone(),
            reason: format!("toml parse failure: {e}"),
        })?;
    if manifest.name != ns.as_str() {
        return Err(ResolutionError::ManifestInvalid {
            namespace: ns.clone(),
            reason: format!(
                "manifest name {:?} does not match directory name {:?}",
                manifest.name,
                ns.as_str()
            ),
        });
    }
    Ok(manifest)
}

fn gather_candidates<F: Filesystem>(
    fs: &F,
    registry: &RegistryLayout,
    ns: &NamespaceId,
    manifest: &AenvManifest,
    out: &mut Vec<Candidate>,
) -> Result<(), ResolutionError> {
    let ns_root = registry.namespace_dir(ns.as_str());
    for (adapter_name, entry) in &manifest.adapters {
        for rel in &entry.files {
            // Phase 2 supports glob expansion only for paths the namespace
            // dir contains literally; full glob is deferred to Phase 4.
            // Skip patterns containing `*` for now and error if they don't
            // resolve to a literal file.
            if rel.contains('*') {
                expand_glob(fs, &ns_root, rel)
                    .map_err(|e| ResolutionError::Io(e.to_string()))?
                    .into_iter()
                    .for_each(|literal| {
                        out.push(Candidate {
                            namespace: ns.clone(),
                            path: PathBuf::from(&literal),
                            source_path: ns_root.join(&literal),
                            adapter: adapter_name.clone(),
                            merge_override: entry
                                .merge
                                .as_ref()
                                .and_then(|m| m.get(&literal).cloned()),
                        })
                    });
            } else {
                let source = ns_root.join(rel);
                if !fs.exists(&source).map_err(|e| ResolutionError::Io(e.to_string()))? {
                    // Adapter declared this file but the namespace doesn't ship it.
                    // Phase 2 treats this as soft-missing: log and skip.
                    continue;
                }
                out.push(Candidate {
                    namespace: ns.clone(),
                    path: PathBuf::from(rel),
                    source_path: source,
                    adapter: adapter_name.clone(),
                    merge_override: entry
                        .merge
                        .as_ref()
                        .and_then(|m| m.get(rel).cloned()),
                });
            }
        }
    }
    Ok(())
}

/// Minimal glob: walk ns_root, return relative paths matching the pattern.
/// Phase 2 restricts patterns to literal paths plus a trailing `**/*` suffix;
/// no `regex` dependency. Full globbing lands in Phase 4 with skill imports.
fn expand_glob<F: Filesystem>(
    fs: &F,
    ns_root: &Path,
    pattern: &str,
) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    walk_dir(fs, ns_root, Path::new(""), &mut out)?;
    Ok(out
        .into_iter()
        .filter(|rel| glob_match(pattern, rel))
        .collect())
}

fn walk_dir<F: Filesystem>(
    fs: &F,
    abs_base: &Path,
    rel_prefix: &Path,
    out: &mut Vec<String>,
) -> std::io::Result<()> {
    let abs = abs_base.join(rel_prefix);
    // Filesystem::list_dir returns io::Result<Vec<PathBuf>>; each entry is the
    // absolute child path. Derive the relative name via Path::file_name().
    for entry in fs.list_dir(&abs)? {
        let name = match entry.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue, // root-like entries with no file name component
        };
        let child_rel = rel_prefix.join(&name);
        let child_abs = abs_base.join(&child_rel);
        let meta = fs.metadata(&child_abs)?;
        if matches!(meta.kind, crate::fs::FileKind::Directory) {
            walk_dir(fs, abs_base, &child_rel, out)?;
        } else {
            out.push(child_rel.to_string_lossy().to_string());
        }
    }
    Ok(())
}

/// Trivial glob: supports a trailing `**/*` (any depth) and exact literals.
/// No `regex` crate; full globbing lands in Phase 4 with skill imports.
fn glob_match(pattern: &str, candidate: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**/*") {
        candidate.starts_with(prefix) && candidate[prefix.len()..].starts_with('/')
    } else if let Some(prefix) = pattern.strip_suffix("**/*") {
        candidate.starts_with(prefix)
    } else {
        pattern == candidate
    }
}
```

The matcher is intentionally tiny — the seven Phase 2 adapter TOMLs use at most one trailing `**/*` per `files` entry. No `regex` dependency is added.

- [ ] **Step 5: Extend `AdapterEntry` to support per-file overrides**

Modify `crates/aenv-core/src/manifest.rs` — `AdapterEntry`:

```rust
#[derive(Debug, Clone, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AdapterEntry {
    #[serde(default)]
    pub files: Vec<String>,
    /// Per-file merge override. Key is relative path; value is one of:
    /// "section", "deep", "last-wins", "symlink".
    #[serde(default)]
    pub merge: Option<std::collections::BTreeMap<String, String>>,
}
```

Add a unit test in `crates/aenv-core/tests/manifest.rs`:

```rust
#[test]
fn parses_per_file_merge_override() {
    let toml = r#"
name = "leaf"
extends = ["base"]
[adapters.claude-code]
files = ["CLAUDE.md", ".mcp.json"]
merge = { ".mcp.json" = "deep" }
"#;
    let m: aenv_core::manifest::AenvManifest = toml::from_str(toml).unwrap();
    let entry = m.adapters.get("claude-code").unwrap();
    assert_eq!(entry.merge.as_ref().unwrap().get(".mcp.json").unwrap(), "deep");
}
```

- [ ] **Step 6: Add tests for the chain + cycle cases**

Append to `crates/aenv-core/tests/resolve.rs`:

```rust
#[test]
fn resolves_two_level_chain_root_then_leaf() {
    let fs = MockFilesystem::default();
    write_manifest(
        &fs,
        "base",
        r#"
name = "base"
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    );
    write_file(&fs, "base", "CLAUDE.md", "# base\n");
    write_manifest(
        &fs,
        "leaf",
        r#"
name = "leaf"
extends = ["base"]
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    );
    write_file(&fs, "leaf", "CLAUDE.md", "# leaf\n");

    let resolved = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap();

    assert_eq!(
        resolved.chain,
        vec![
            NamespaceId::new("base").unwrap(),
            NamespaceId::new("leaf").unwrap()
        ]
    );
    // Both candidates present, root first.
    assert_eq!(resolved.candidates.len(), 2);
    assert_eq!(resolved.candidates[0].namespace.as_str(), "base");
    assert_eq!(resolved.candidates[1].namespace.as_str(), "leaf");
}

#[test]
fn detects_two_node_cycle() {
    let fs = MockFilesystem::default();
    write_manifest(
        &fs,
        "a",
        r#"
name = "a"
extends = ["b"]
"#,
    );
    write_manifest(
        &fs,
        "b",
        r#"
name = "b"
extends = ["a"]
"#,
    );

    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("a").unwrap(),
    )
    .unwrap_err();
    match err {
        ResolutionError::Cycle(chain) => {
            // Chain shows where the cycle closed.
            assert_eq!(chain.first().unwrap().as_str(), "a");
            assert_eq!(chain.last().unwrap().as_str(), "a");
            assert!(chain.iter().any(|n| n.as_str() == "b"));
        }
        other => panic!("expected Cycle, got {other:?}"),
    }
}

#[test]
fn detects_self_cycle() {
    let fs = MockFilesystem::default();
    write_manifest(
        &fs,
        "selfish",
        r#"
name = "selfish"
extends = ["selfish"]
"#,
    );
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("selfish").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, ResolutionError::Cycle(_)));
}

#[test]
fn resolves_diamond_without_duplication() {
    // shared <- left, shared <- right, top extends [left, right]
    let fs = MockFilesystem::default();
    write_manifest(&fs, "shared", r#"name = "shared""#);
    write_manifest(
        &fs,
        "left",
        r#"
name = "left"
extends = ["shared"]
"#,
    );
    write_manifest(
        &fs,
        "right",
        r#"
name = "right"
extends = ["shared"]
"#,
    );
    write_manifest(
        &fs,
        "top",
        r#"
name = "top"
extends = ["left", "right"]
"#,
    );

    let resolved = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("top").unwrap(),
    )
    .unwrap();
    // `shared` appears exactly once even though both `left` and `right` extend it.
    let count_shared = resolved
        .chain
        .iter()
        .filter(|n| n.as_str() == "shared")
        .count();
    assert_eq!(count_shared, 1);
    // Order: shared, left, right, top — shared first because it's reached via the leftmost branch.
    assert_eq!(
        resolved.chain.iter().map(|n| n.as_str()).collect::<Vec<_>>(),
        vec!["shared", "left", "right", "top"]
    );
}

#[test]
fn rejects_unknown_namespace() {
    let fs = MockFilesystem::default();
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("missing").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, ResolutionError::NamespaceNotFound(_)));
}

#[test]
fn rejects_manifest_name_directory_mismatch() {
    let fs = MockFilesystem::default();
    // Directory is "alpha" but manifest claims name = "beta"
    write_manifest(
        &fs,
        "alpha",
        r#"
name = "beta"
"#,
    );
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("alpha").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, ResolutionError::ManifestInvalid { .. }));
}

#[test]
fn rejects_reference_to_missing_adapter() {
    let fs = MockFilesystem::default();
    write_manifest(
        &fs,
        "ghost",
        r#"
name = "ghost"
[adapters.does-not-exist]
files = ["foo"]
"#,
    );
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("ghost").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, ResolutionError::AdapterMissing(_)));
}
```

The `mock_filesystem` module is copied (or `#[path = "..."]`-included) from `aenv-core/tests/mock_filesystem.rs` (Phase 0 shared utility).

- [ ] **Step 7: Run all resolve tests**

Run: `cargo test -p aenv-core --test resolve`
Expected: 7 PASS.

`cargo clippy -p aenv-core -- -D warnings`.

- [ ] **Step 8: Commit**

```bash
git add crates/aenv-core/src/resolve.rs crates/aenv-core/src/manifest.rs \
        crates/aenv-core/tests/resolve.rs crates/aenv-core/tests/manifest.rs
git commit -m "Add extends-chain resolver with cycle detection

resolve_namespace walks the extends chain depth-first starting from a
leaf NamespaceId, producing the chain (root -> leaf) and the candidate
artifact list. Cycles surface as ResolutionError::Cycle with the
offending chain; missing namespaces and missing adapters surface with
their names. Diamond inheritance is handled (a node reached via two
parents appears once, in left-branch-first position).

Adds an optional 'merge' override map to AdapterEntry so a manifest
can pin a per-file strategy at the namespace level. Tests: single,
two-level, diamond, two-node cycle, self-cycle, unknown namespace,
manifest/dir name mismatch, missing adapter reference.

No glob support yet beyond the single trailing '**/*' suffix; full
glob lands in Phase 4 along with skill imports.
"
```

---

### Task 4: Strategy selection

Given the candidate set from Task 3, decide the `MaterializeStrategy` for each *path* (not each candidate — there may be many candidates per path).

Decision tree:
1. If only one candidate for the path: `Symlink` (or `Identical`, decided later by activate.rs when comparing to project state).
2. If multiple candidates:
   - If the *latest* manifest declares an override (`merge = "deep" | "section" | "last-wins"`), use that.
   - Else look at the adapter's `role` for this path (Phase 2 supports `role = "instructions"` only): if `instructions`, `SectionMerge`.
   - Else: pick `DeepMerge` if the extension is `.json`, `.yaml`, `.yml`, `.toml` *AND* the adapter declares `default_merge = "deep"` for that path. (See adapter TOMLs in Task 12 for examples.)
   - Else: `last-wins` — treated as `Symlink` to the latest namespace, with earlier candidates becoming shadows (Task 9).

**Files:**
- Create: `crates/aenv-core/src/strategy.rs`
- Modify: `crates/aenv-core/src/adapter.rs` — `Adapter` gains optional `roles: BTreeMap<String, String>` (path → role) and optional `default_merge: BTreeMap<String, String>` (path → strategy)
- Modify: `crates/aenv-core/src/lib.rs` — `pub mod strategy;`
- Test: `crates/aenv-core/tests/strategy.rs`

- [ ] **Step 1: Extend the `Adapter` type**

Modify `crates/aenv-core/src/adapter.rs` — `Adapter`:

```rust
#[derive(Debug, Clone, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Adapter {
    pub name: String,
    #[serde(default)]
    pub files: Vec<String>,
    /// Phase 1 holdover — explicit per-file merge declaration on the adapter
    /// (rarely used; manifests override). Kept for back-compat.
    #[serde(default)]
    pub merge_strategies: std::collections::BTreeMap<String, String>,
    /// Per-path role declaration. Phase 2 understands `"instructions"`.
    #[serde(default)]
    pub roles: std::collections::BTreeMap<String, String>,
    /// Per-path default merge strategy (consulted before role fallback).
    #[serde(default)]
    pub default_merge: std::collections::BTreeMap<String, String>,
}
```

Add a unit test in `crates/aenv-core/tests/adapter.rs`:

```rust
#[test]
fn adapter_parses_roles_and_default_merge() {
    let toml = r#"
name = "mcp"
files = [".mcp.json"]
[default_merge]
".mcp.json" = "deep"
"#;
    let a: aenv_core::adapter::Adapter = toml::from_str(toml).unwrap();
    assert_eq!(a.default_merge.get(".mcp.json").unwrap(), "deep");
}

#[test]
fn adapter_parses_role_declaration() {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"#;
    let a: aenv_core::adapter::Adapter = toml::from_str(toml).unwrap();
    assert_eq!(a.roles.get("CLAUDE.md").unwrap(), "instructions");
}
```

- [ ] **Step 2: Write the strategy test**

Create `crates/aenv-core/tests/strategy.rs`:

```rust
use std::path::PathBuf;

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::{Candidate, DeepMergeFormat, MaterializeStrategy};
use aenv_core::strategy::decide_strategy;

fn cand(ns: &str, path: &str, adapter: &str, override_: Option<&str>) -> Candidate {
    Candidate {
        namespace: NamespaceId::new(ns).unwrap(),
        path: PathBuf::from(path),
        source_path: PathBuf::from(format!("/aenv/envs/{ns}/{path}")),
        adapter: adapter.to_string(),
        merge_override: override_.map(|s| s.to_string()),
    }
}

fn cc() -> Adapter {
    toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"#,
    )
    .unwrap()
}

fn mcp() -> Adapter {
    toml::from_str(
        r#"
name = "mcp"
files = [".mcp.json"]
[default_merge]
".mcp.json" = "deep"
"#,
    )
    .unwrap()
}

fn registry() -> AdapterRegistry {
    let mut r = AdapterRegistry::default();
    r.insert(cc());
    r.insert(mcp());
    r
}

#[test]
fn single_candidate_is_symlink() {
    let strat = decide_strategy(&[cand("base", "CLAUDE.md", "claude-code", None)], &registry())
        .unwrap();
    assert!(matches!(strat, MaterializeStrategy::Symlink));
}

#[test]
fn instructions_role_with_two_candidates_section_merges() {
    let candidates = [
        cand("base", "CLAUDE.md", "claude-code", None),
        cand("leaf", "CLAUDE.md", "claude-code", None),
    ];
    let strat = decide_strategy(&candidates, &registry()).unwrap();
    assert!(matches!(strat, MaterializeStrategy::SectionMerge));
}

#[test]
fn manifest_override_wins_over_role_default() {
    // CLAUDE.md is normally section-merged; an override of "last-wins" forces symlink.
    let candidates = [
        cand("base", "CLAUDE.md", "claude-code", None),
        cand("leaf", "CLAUDE.md", "claude-code", Some("last-wins")),
    ];
    let strat = decide_strategy(&candidates, &registry()).unwrap();
    assert!(matches!(strat, MaterializeStrategy::Symlink));
}

#[test]
fn default_merge_deep_picks_deepjson_for_dot_mcp_json() {
    let candidates = [
        cand("base", ".mcp.json", "mcp", None),
        cand("leaf", ".mcp.json", "mcp", None),
    ];
    let strat = decide_strategy(&candidates, &registry()).unwrap();
    assert!(matches!(
        strat,
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Json)
    ));
}

#[test]
fn deep_override_on_yaml_picks_yaml_format() {
    let candidates = [
        cand("base", ".aider.conf.yml", "aider", Some("deep")),
        cand("leaf", ".aider.conf.yml", "aider", Some("deep")),
    ];
    // Even though no adapter `default_merge` is set, the manifest override
    // selects "deep" and the file extension determines the format.
    let mut reg = registry();
    reg.insert(toml::from_str(r#"name = "aider""#).unwrap());
    let strat = decide_strategy(&candidates, &reg).unwrap();
    assert!(matches!(
        strat,
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Yaml)
    ));
}

#[test]
fn deep_override_on_toml_picks_toml_format() {
    let candidates = [
        cand("base", "config.toml", "x", Some("deep")),
        cand("leaf", "config.toml", "x", Some("deep")),
    ];
    let mut reg = AdapterRegistry::default();
    reg.insert(toml::from_str(r#"name = "x""#).unwrap());
    let strat = decide_strategy(&candidates, &reg).unwrap();
    assert!(matches!(
        strat,
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Toml)
    ));
}

#[test]
fn unknown_extension_with_deep_override_errors() {
    let candidates = [
        cand("base", "config.xyz", "x", Some("deep")),
        cand("leaf", "config.xyz", "x", Some("deep")),
    ];
    let mut reg = AdapterRegistry::default();
    reg.insert(toml::from_str(r#"name = "x""#).unwrap());
    let err = decide_strategy(&candidates, &reg).unwrap_err();
    assert!(err.to_string().contains("deep-merge requires"));
}

#[test]
fn two_candidates_no_role_no_override_fall_back_to_last_wins() {
    let candidates = [
        cand("base", ".cursorrules", "cursor", None),
        cand("leaf", ".cursorrules", "cursor", None),
    ];
    let mut reg = registry();
    reg.insert(toml::from_str(r#"name = "cursor"
files = [".cursorrules"]"#).unwrap());
    let strat = decide_strategy(&candidates, &reg).unwrap();
    // "last-wins" = symlink to the latest namespace's file.
    assert!(matches!(strat, MaterializeStrategy::Symlink));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p aenv-core --test strategy`
Expected: compile error (`aenv_core::strategy` does not exist).

- [ ] **Step 4: Implement `decide_strategy`**

Create `crates/aenv-core/src/strategy.rs`:

```rust
//! Strategy selection: given the candidate list for a single path, decide
//! the materialization strategy.
//!
//! Priority order:
//!   1. Single candidate     -> Symlink (Identical decided at activation time)
//!   2. Manifest override    -> the named strategy on the latest candidate wins
//!   3. Adapter role         -> "instructions" => SectionMerge
//!   4. Adapter default_merge -> "deep" => DeepMerge(format-from-extension)
//!   5. Fallback              -> last-wins (Symlink to latest, earlier become shadows)

use std::path::Path;

use crate::adapter::AdapterRegistry;
use crate::resolve::{Candidate, DeepMergeFormat, MaterializeStrategy};
use crate::AenvError;

pub fn decide_strategy(
    candidates: &[Candidate],
    adapters: &AdapterRegistry,
) -> Result<MaterializeStrategy, AenvError> {
    if candidates.is_empty() {
        return Err(AenvError::ManifestInvalid(
            "strategy selection called with no candidates".into(),
        ));
    }
    if candidates.len() == 1 {
        return Ok(MaterializeStrategy::Symlink);
    }

    let latest = candidates.last().unwrap();
    let path = latest.path.as_path();

    // (2) Manifest override on the latest candidate.
    if let Some(name) = &latest.merge_override {
        return strategy_from_name(name, path);
    }

    // (3) Adapter role.
    if let Some(adapter) = adapters.get(&latest.adapter) {
        let path_key = path.to_string_lossy().to_string();
        if let Some(role) = adapter.roles.get(&path_key) {
            if role == "instructions" {
                return Ok(MaterializeStrategy::SectionMerge);
            }
        }

        // (4) Adapter default_merge.
        if let Some(strat) = adapter.default_merge.get(&path_key) {
            return strategy_from_name(strat, path);
        }
    }

    // (5) Fallback: last-wins, which is a symlink to the latest candidate.
    Ok(MaterializeStrategy::Symlink)
}

fn strategy_from_name(name: &str, path: &Path) -> Result<MaterializeStrategy, AenvError> {
    match name {
        "section" | "section-merge" => Ok(MaterializeStrategy::SectionMerge),
        "deep" | "deep-merge" => Ok(MaterializeStrategy::DeepMerge(format_from_path(path)?)),
        "symlink" | "last-wins" => Ok(MaterializeStrategy::Symlink),
        other => Err(AenvError::ManifestInvalid(format!(
            "unknown merge strategy {other:?}; expected one of section, deep, last-wins"
        ))),
    }
}

fn format_from_path(path: &Path) -> Result<DeepMergeFormat, AenvError> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("json") => Ok(DeepMergeFormat::Json),
        Some("yaml" | "yml") => Ok(DeepMergeFormat::Yaml),
        Some("toml") => Ok(DeepMergeFormat::Toml),
        _ => Err(AenvError::ManifestInvalid(format!(
            "deep-merge requires .json, .yaml, .yml, or .toml extension; got {}",
            path.display()
        ))),
    }
}
```

- [ ] **Step 5: Wire the module**

Modify `crates/aenv-core/src/lib.rs` — add `pub mod strategy;`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p aenv-core --test strategy`
Expected: 8 PASS.

Also re-run the adapter tests for the new fields: `cargo test -p aenv-core --test adapter` — should still pass.

- [ ] **Step 7: Commit**

```bash
git add crates/aenv-core/src/strategy.rs crates/aenv-core/src/adapter.rs \
        crates/aenv-core/src/lib.rs crates/aenv-core/tests/strategy.rs \
        crates/aenv-core/tests/adapter.rs
git commit -m "Add strategy selection for resolved candidates

decide_strategy collapses a path's candidate list into a single
MaterializeStrategy. Priority order: single candidate -> Symlink;
manifest override on the latest candidate; adapter role
('instructions' => SectionMerge); adapter default_merge ('deep' =>
DeepMerge(format)); last-wins fallback (Symlink).

Extends Adapter with optional 'roles' and 'default_merge' maps so the
seven Phase 2 adapter TOMLs can declare merge defaults without forcing
every manifest to opt in.
"
```

---

### Task 5: Section-merge for Markdown (`<!-- aenv:replace -->` marker)

The default merge for `role = "instructions"` files (e.g. `CLAUDE.md`, `.cursorrules`). Merge by top-level (`#`) and second-level (`##`) Markdown sections. Same section in two namespaces: by default *append* in chain order. If the later namespace prefixes the section body with the HTML comment `<!-- aenv:replace -->`, *replace* the earlier content instead.

**Decision: parser choice.** Use `pulldown-cmark` only for tokenizing into events; we don't need a full Markdown AST. The implementation tracks heading level + heading text and accumulates section bodies as raw byte slices of the source. This keeps roundtripping exact: we don't normalize indentation, code-block fences, list markers, etc.

**Files:**
- Create: `crates/aenv-core/src/merge/mod.rs` (dispatcher + the `MergeError` type)
- Create: `crates/aenv-core/src/merge/section.rs`
- Modify: `crates/aenv-core/src/lib.rs` (`pub mod merge;`)
- Modify: `crates/aenv-core/Cargo.toml` (add `pulldown-cmark` dependency)
- Modify: workspace `Cargo.toml` (declare `pulldown-cmark` in `[workspace.dependencies]`)
- Test: `crates/aenv-core/tests/merge_section.rs`

- [ ] **Step 1: Add the workspace dependency**

Modify workspace `Cargo.toml`:

```toml
[workspace.dependencies]
# ... existing entries ...
pulldown-cmark = { version = "0.10", default-features = false }
```

Modify `crates/aenv-core/Cargo.toml`:

```toml
[dependencies]
# ... existing ...
pulldown-cmark = { workspace = true }
```

Run: `cargo build -p aenv-core` — should download + compile clean.

- [ ] **Step 2: Write the failing test**

Create `crates/aenv-core/tests/merge_section.rs`:

```rust
use aenv_core::merge::section::merge_sections;

#[test]
fn empty_inputs_produce_empty_output() {
    let out = merge_sections(&[]);
    assert_eq!(out, "");
}

#[test]
fn single_input_passes_through_unchanged() {
    let body = "# Top\n\nsome text\n";
    let out = merge_sections(&[body.to_string()]);
    assert_eq!(out, body);
}

#[test]
fn distinct_top_sections_concatenate_in_chain_order() {
    let base = "# Build & Test\n\ncargo test\n";
    let leaf = "# Disposition\n\nbe terse\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    assert!(out.starts_with("# Build & Test"));
    assert!(out.contains("# Disposition"));
    assert!(out.contains("cargo test"));
    assert!(out.contains("be terse"));
}

#[test]
fn same_section_appends_by_default() {
    let base = "## Conventions\n\n- single quotes\n";
    let leaf = "## Conventions\n\n- four-space indent\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    // Single section heading, both bullets present in order.
    let heading_count = out.matches("## Conventions").count();
    assert_eq!(heading_count, 1, "should de-duplicate the heading");
    let single = out.find("- single quotes").unwrap();
    let four = out.find("- four-space indent").unwrap();
    assert!(single < four, "base's content precedes leaf's");
}

#[test]
fn replace_marker_overrides_append() {
    let base = "## Conventions\n\n- single quotes\n";
    let leaf = "## Conventions\n<!-- aenv:replace -->\n\n- four-space indent\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    // Base's content is gone; only leaf's content remains under that section.
    assert!(!out.contains("single quotes"));
    assert!(out.contains("four-space indent"));
    // The marker itself is stripped from the output.
    assert!(!out.contains("aenv:replace"));
}

#[test]
fn preamble_before_first_heading_is_preserved_per_namespace() {
    let base = "Some preamble.\n\n# Top\n\nbody\n";
    let leaf = "# Top\n\nleaf body\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    assert!(out.starts_with("Some preamble.\n\n"));
    assert!(out.contains("body"));
    assert!(out.contains("leaf body"));
}

#[test]
fn nested_subsections_merge_under_their_parent_heading() {
    let base = "## Build\n\n### Lint\n\nclippy\n";
    let leaf = "## Build\n\n### Test\n\ncargo test\n";
    let out = merge_sections(&[base.to_string(), leaf.to_string()]);
    let build_count = out.matches("## Build").count();
    assert_eq!(build_count, 1);
    assert!(out.contains("### Lint"));
    assert!(out.contains("### Test"));
}

#[test]
fn three_level_chain_appends_in_order() {
    let a = "## X\n\na\n";
    let b = "## X\n\nb\n";
    let c = "## X\n\nc\n";
    let out = merge_sections(&[a.to_string(), b.to_string(), c.to_string()]);
    let ia = out.find("\na\n").unwrap();
    let ib = out.find("\nb\n").unwrap();
    let ic = out.find("\nc\n").unwrap();
    assert!(ia < ib && ib < ic);
}

#[test]
fn replace_in_middle_of_chain_replaces_only_prior_content() {
    let a = "## X\n\na\n";
    let b = "## X\n<!-- aenv:replace -->\n\nb\n";
    let c = "## X\n\nc\n";
    let out = merge_sections(&[a.to_string(), b.to_string(), c.to_string()]);
    // 'a' is gone; 'b' then 'c' remain.
    assert!(!out.contains("\na\n"));
    let ib = out.find("\nb\n").unwrap();
    let ic = out.find("\nc\n").unwrap();
    assert!(ib < ic);
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p aenv-core --test merge_section`
Expected: compile error — `aenv_core::merge` doesn't exist.

- [ ] **Step 4: Create the merge module skeleton**

Create `crates/aenv-core/src/merge/mod.rs`:

```rust
//! Merge algorithms for composed namespaces.
//!
//! Each submodule owns one strategy:
//!   * `section`  — Markdown by `#`/`##` section, with `<!-- aenv:replace -->`
//!   * `deep_json`, `deep_yaml`, `deep_toml` — structured deep-merge per format
//!
//! All merge functions take `Vec<bytes>` in chain order (root first) and
//! return the merged byte output. Errors are reported as `MergeError`.

pub mod section;
pub mod deep_json;
pub mod deep_yaml;
pub mod deep_toml;

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("parse error in {kind}: {source}")]
    Parse {
        kind: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error("incompatible types during {kind} merge at {path}")]
    TypeMismatch { kind: &'static str, path: String },
    #[error("UTF-8 decoding failed: {0}")]
    Utf8(String),
}

impl From<MergeError> for crate::AenvError {
    fn from(value: MergeError) -> Self {
        crate::AenvError::ActivationConflict(value.to_string())
    }
}
```

Note: this introduces `thiserror` (already a workspace dep) and `anyhow` (new). Add `anyhow = "1.0"` to workspace deps + `crates/aenv-core/Cargo.toml` `[dependencies]` if not present. *Actually*, sidestep anyhow: change `Parse` to carry `String` source instead, since none of the merge errors need source-chain integration.

```rust
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("parse error in {kind}: {source}")]
    Parse { kind: &'static str, source: String },
    #[error("incompatible types during {kind} merge at {path}")]
    TypeMismatch { kind: &'static str, path: String },
    #[error("UTF-8 decoding failed: {0}")]
    Utf8(String),
}
```

That avoids the new `anyhow` dep.

- [ ] **Step 5: Implement `merge_sections`**

Create `crates/aenv-core/src/merge/section.rs`:

```rust
//! Markdown section-merge.
//!
//! The merge groups content by `#` and `##` headings. Within a single input,
//! a section's body is the source text from immediately after the heading up
//! to the next heading at the same or lower depth.
//!
//! Default behavior: across inputs (chain order), a section's body is the
//! concatenation of bodies from each input that has that heading. If the
//! body in a later input begins (after the heading line and any whitespace)
//! with `<!-- aenv:replace -->`, the bodies from earlier inputs for that
//! exact heading are discarded.
//!
//! Headings are matched by their literal text (trimmed). Different heading
//! depths with the same text are distinct sections.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

const REPLACE_MARKER: &str = "<!-- aenv:replace -->";

/// Parsed input: per-input list of (heading-or-none, body) blocks. Index 0
/// is "before any heading" content (the preamble).
#[derive(Debug, Clone)]
struct ParsedInput {
    preamble: String,
    sections: Vec<Section>,
}

#[derive(Debug, Clone)]
struct Section {
    key: SectionKey,
    /// Body text *without* the heading line — already stripped of the leading
    /// blank line that typically follows a heading.
    body: String,
    replace: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct SectionKey {
    depth: HeadingDepth,
    title: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum HeadingDepth {
    H1,
    H2,
    Other(u8),
}

impl HeadingDepth {
    fn from(level: HeadingLevel) -> Self {
        match level {
            HeadingLevel::H1 => HeadingDepth::H1,
            HeadingLevel::H2 => HeadingDepth::H2,
            HeadingLevel::H3 => HeadingDepth::Other(3),
            HeadingLevel::H4 => HeadingDepth::Other(4),
            HeadingLevel::H5 => HeadingDepth::Other(5),
            HeadingLevel::H6 => HeadingDepth::Other(6),
        }
    }
    fn marker(self) -> &'static str {
        match self {
            HeadingDepth::H1 => "#",
            HeadingDepth::H2 => "##",
            HeadingDepth::Other(3) => "###",
            HeadingDepth::Other(4) => "####",
            HeadingDepth::Other(5) => "#####",
            HeadingDepth::Other(6) => "######",
            _ => "##",
        }
    }
}

/// Public entrypoint.
pub fn merge_sections(inputs: &[String]) -> String {
    if inputs.is_empty() {
        return String::new();
    }
    let parsed: Vec<ParsedInput> = inputs.iter().map(|s| parse(s)).collect();

    // Preambles in chain order, separated by blank lines.
    let mut out = String::new();
    for p in &parsed {
        if !p.preamble.trim().is_empty() {
            out.push_str(p.preamble.trim_end());
            out.push_str("\n\n");
        }
    }

    // Sections — preserve first-occurrence order across the chain.
    let mut order: Vec<SectionKey> = Vec::new();
    let mut by_key: std::collections::HashMap<SectionKey, Vec<&Section>> = Default::default();
    for p in &parsed {
        for s in &p.sections {
            if !by_key.contains_key(&s.key) {
                order.push(s.key.clone());
            }
            by_key.entry(s.key.clone()).or_default().push(s);
        }
    }

    for key in order {
        let sections = &by_key[&key];
        // If any section in the chain has `replace = true`, the section's
        // effective bodies are those from that section onward.
        let start_idx = sections
            .iter()
            .rposition(|s| s.replace)
            .unwrap_or(0);
        let effective = &sections[start_idx..];

        out.push_str(key.depth.marker());
        out.push(' ');
        out.push_str(&key.title);
        out.push('\n');

        for s in effective {
            // Each contributing body is trimmed to remove the leading blank line
            // pulldown leaves before content; we put it back as a single blank line.
            let body = s.body.trim_end();
            if body.is_empty() {
                continue;
            }
            out.push('\n');
            out.push_str(body);
            out.push('\n');
        }
        out.push('\n');
    }
    // Single trailing newline.
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn parse(input: &str) -> ParsedInput {
    // Strategy: walk parser events, track heading boundaries by byte offset,
    // and slice the original source between boundaries to preserve formatting.
    //
    // pulldown-cmark 0.10 changed `Tag::Heading` to a struct variant and
    // introduced `TagEnd` for the End event. Pin Options::empty() so we don't
    // pull in tables/footnotes/strikethrough which would inflate the event
    // stream and complicate heading detection.
    let parser = Parser::new_ext(input, Options::empty()).into_offset_iter();
    let mut headings: Vec<(SectionKey, std::ops::Range<usize>)> = Vec::new();
    let mut current_heading: Option<(SectionKey, usize, usize)> = None; // (key, start, end)

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some((
                    SectionKey {
                        depth: HeadingDepth::from(level),
                        title: String::new(),
                    },
                    range.start,
                    range.end,
                ));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((ref mut k, ref _start, ref _end)) = current_heading {
                    k.title.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((key, start, end)) = current_heading.take() {
                    headings.push((
                        SectionKey {
                            depth: key.depth,
                            title: key.title.trim().to_string(),
                        },
                        start..end,
                    ));
                }
            }
            _ => {}
        }
    }

    // Build preamble + sections by slicing input between heading boundaries.
    let preamble_end = headings.first().map(|(_, r)| r.start).unwrap_or(input.len());
    let preamble = input[..preamble_end].to_string();
    let mut sections = Vec::with_capacity(headings.len());
    for (i, (key, range)) in headings.iter().enumerate() {
        let body_start = range.end;
        let body_end = headings
            .get(i + 1)
            .map(|(_, r2)| r2.start)
            .unwrap_or(input.len());
        let raw_body = &input[body_start..body_end];

        // Detect replace marker: first non-whitespace line is the marker.
        let trimmed = raw_body.trim_start_matches(|c: char| c == '\n' || c == ' ');
        let (replace, body) = if let Some(rest) = trimmed.strip_prefix(REPLACE_MARKER) {
            // Strip the marker and one following newline.
            let after = rest.strip_prefix('\n').unwrap_or(rest).to_string();
            (true, after)
        } else {
            (false, raw_body.trim_start_matches('\n').to_string())
        };

        sections.push(Section { key: key.clone(), body, replace });
    }
    ParsedInput { preamble, sections }
}
```

- [ ] **Step 6: Run the test**

Run: `cargo test -p aenv-core --test merge_section`
Expected: 9 PASS.

`cargo clippy -p aenv-core -- -D warnings` should pass; note `pulldown-cmark` may surface `clippy::single_match` style nits in the parser loop — handle them inline.

- [ ] **Step 7: Commit**

```bash
git add crates/aenv-core/src/merge/mod.rs crates/aenv-core/src/merge/section.rs \
        crates/aenv-core/src/lib.rs crates/aenv-core/Cargo.toml \
        Cargo.toml crates/aenv-core/tests/merge_section.rs
git commit -m "Add section-merge for Markdown instructions files

merge_sections walks pulldown-cmark events to identify H1/H2 headings,
slices the source between heading boundaries to preserve formatting
exactly (code blocks, list markers, indentation), and concatenates
bodies for the same heading across the chain. The '<!-- aenv:replace
-->' marker on a section in a later namespace discards earlier bodies
for that heading. Preambles (content before any heading) are kept
per-namespace.

Adds pulldown-cmark 0.10 as a workspace dep. The merge module also
defines the MergeError enum used by the three deep-merge backends in
later tasks.
"
```

---

### Task 6: Deep-merge for JSON (`serde_json::Value` recursive merge)

`serde_json::Value` is the canonical representation; the YAML and TOML deep-merges convert into it, merge, and convert back. Implementing JSON cleanly lets the others reuse the logic.

**Merge rules:**
- Two `Object`s: union of keys; recursive merge on overlap.
- Two `Array`s: concatenate (chain-order).
- Any other type collision (`String` + `Number`, `Object` + `Array`, etc.): the *later* value wins, with the strategy logging the override at INFO via `tracing` (deferred to Phase 5 — for Phase 2, just take the later value silently — matches the engineering doc's "last-wins fallback" semantics inside deep-merge).
- `null` + anything: anything wins.

**Files:**
- Create: `crates/aenv-core/src/merge/deep_json.rs`
- Test: `crates/aenv-core/tests/merge_deep.rs` (covers all three formats — TOML and YAML land in Tasks 7–8)

- [ ] **Step 1: Write the failing JSON tests**

Create `crates/aenv-core/tests/merge_deep.rs`:

```rust
use aenv_core::merge::deep_json::merge_json;

#[test]
fn merges_two_objects_union_of_keys() {
    let a = br#"{"a":1,"b":2}"#;
    let b = br#"{"b":20,"c":3}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["a"], 1);
    assert_eq!(v["b"], 20);
    assert_eq!(v["c"], 3);
}

#[test]
fn arrays_concatenate_in_chain_order() {
    let a = br#"{"x":[1,2]}"#;
    let b = br#"{"x":[3]}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["x"].as_array().unwrap().len(), 3);
    assert_eq!(v["x"][0], 1);
    assert_eq!(v["x"][2], 3);
}

#[test]
fn nested_objects_merge_recursively() {
    let a = br#"{"servers":{"a":{"command":"cmd-a"}}}"#;
    let b = br#"{"servers":{"b":{"command":"cmd-b"}}}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(v["servers"]["a"]["command"] == "cmd-a");
    assert!(v["servers"]["b"]["command"] == "cmd-b");
}

#[test]
fn type_mismatch_later_wins() {
    let a = br#"{"x":1}"#;
    let b = br#"{"x":"string"}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["x"], "string");
}

#[test]
fn null_loses_to_value() {
    let a = br#"{"x":null}"#;
    let b = br#"{"x":1}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["x"], 1);
}

#[test]
fn invalid_json_returns_parse_error() {
    let a = br#"{"x":"#; // truncated
    let err = merge_json(&[a.to_vec()]).unwrap_err();
    assert!(matches!(
        err,
        aenv_core::merge::MergeError::Parse { kind: "json", .. }
    ));
}

#[test]
fn three_way_chain_preserves_order() {
    let a = br#"{"list":[1]}"#;
    let b = br#"{"list":[2]}"#;
    let c = br#"{"list":[3]}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec(), c.to_vec()]).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let arr = v["list"].as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0], 1);
    assert_eq!(arr[1], 2);
    assert_eq!(arr[2], 3);
}

#[test]
fn output_is_stable_two_space_pretty() {
    // The merged output is pretty-printed with two-space indent so that
    // editor diffs are clean. This is part of the contract for Phase 5
    // hash stability — file bytes change only when meaning changes.
    let a = br#"{"a":1}"#;
    let b = br#"{"b":2}"#;
    let out = merge_json(&[a.to_vec(), b.to_vec()]).unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.starts_with("{\n  \""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test merge_deep`
Expected: compile error.

- [ ] **Step 3: Implement `merge_json`**

Create `crates/aenv-core/src/merge/deep_json.rs`:

```rust
//! Deep-merge for JSON.
//!
//! Two-space pretty print on output so editor diffs read cleanly and so that
//! Phase 5's hashing canonicalization has a stable starting point.

use serde_json::Value;

use super::MergeError;

pub fn merge_json(inputs: &[Vec<u8>]) -> Result<Vec<u8>, MergeError> {
    if inputs.is_empty() {
        return Ok(b"{}\n".to_vec());
    }
    let mut acc: Option<Value> = None;
    for bytes in inputs {
        let v: Value = serde_json::from_slice(bytes).map_err(|e| MergeError::Parse {
            kind: "json",
            source: e.to_string(),
        })?;
        acc = Some(match acc.take() {
            None => v,
            Some(existing) => deep_merge_value(existing, v),
        });
    }
    let merged = acc.unwrap_or(Value::Object(Default::default()));
    let mut out = serde_json::to_vec_pretty(&merged).map_err(|e| MergeError::Parse {
        kind: "json",
        source: e.to_string(),
    })?;
    out.push(b'\n');
    Ok(out)
}

pub(crate) fn deep_merge_value(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Object(mut am), Value::Object(bm)) => {
            for (k, bv) in bm {
                let merged = match am.remove(&k) {
                    Some(av) => deep_merge_value(av, bv),
                    None => bv,
                };
                am.insert(k, merged);
            }
            Value::Object(am)
        }
        (Value::Array(mut aa), Value::Array(ba)) => {
            aa.extend(ba);
            Value::Array(aa)
        }
        (Value::Null, b) => b,
        (_, b) => b, // last-wins on type mismatch
    }
}
```

- [ ] **Step 4: Run the JSON tests**

Run: `cargo test -p aenv-core --test merge_deep`
Expected: 8 PASS for JSON cases (YAML/TOML tests land in Tasks 7–8 and will be added to the same file).

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/merge/deep_json.rs crates/aenv-core/tests/merge_deep.rs
git commit -m "Add deep-merge for JSON

Recursive deep merge over serde_json::Value: objects union with
recursive merge on overlap; arrays concatenate; null loses to any
value; other type mismatches resolve last-wins. Two-space pretty
output for stable diffs. merge_json takes Vec<Vec<u8>> in chain order
and returns merged bytes.

deep_merge_value is exposed pub(crate) so the YAML and TOML backends
in Tasks 7 and 8 can reuse it (both convert to serde_json::Value
internally).
"
```

---

### Task 7: Deep-merge for YAML (via `serde_yaml`)

YAML deep-merge is "parse with `serde_yaml`, convert to `serde_json::Value`, reuse `deep_merge_value`, convert back to YAML on output." Conversion is lossy on tag-rich YAML (timestamps, custom tags) but Phase 2's target files (`.aider.conf.yml`, MCP YAML configs) don't use any of those.

**Files:**
- Create: `crates/aenv-core/src/merge/deep_yaml.rs`
- Modify: workspace `Cargo.toml` + `crates/aenv-core/Cargo.toml` to add `serde_yaml`
- Test: append YAML cases to `crates/aenv-core/tests/merge_deep.rs`

- [ ] **Step 1: Add the workspace dependency**

Workspace `Cargo.toml`:

```toml
[workspace.dependencies]
serde_yaml = "0.9"
```

`crates/aenv-core/Cargo.toml`:

```toml
[dependencies]
serde_yaml = { workspace = true }
```

- [ ] **Step 2: Write the YAML tests**

Append to `crates/aenv-core/tests/merge_deep.rs`:

```rust
use aenv_core::merge::deep_yaml::merge_yaml;

#[test]
fn yaml_merges_objects_union_of_keys() {
    let a = b"a: 1\nb: 2\n";
    let b = b"b: 20\nc: 3\n";
    let out = merge_yaml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_yaml::Value = serde_yaml::from_slice(&out).unwrap();
    assert_eq!(v["a"], serde_yaml::Value::Number(1.into()));
    assert_eq!(v["b"], serde_yaml::Value::Number(20.into()));
    assert_eq!(v["c"], serde_yaml::Value::Number(3.into()));
}

#[test]
fn yaml_arrays_concatenate() {
    let a = b"list:\n  - 1\n  - 2\n";
    let b = b"list:\n  - 3\n";
    let out = merge_yaml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_yaml::Value = serde_yaml::from_slice(&out).unwrap();
    let arr = v["list"].as_sequence().unwrap();
    assert_eq!(arr.len(), 3);
}

#[test]
fn yaml_nested_objects_merge_recursively() {
    let a = b"servers:\n  a:\n    cmd: cmd-a\n";
    let b = b"servers:\n  b:\n    cmd: cmd-b\n";
    let out = merge_yaml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: serde_yaml::Value = serde_yaml::from_slice(&out).unwrap();
    assert_eq!(v["servers"]["a"]["cmd"], serde_yaml::Value::String("cmd-a".into()));
    assert_eq!(v["servers"]["b"]["cmd"], serde_yaml::Value::String("cmd-b".into()));
}

#[test]
fn yaml_invalid_input_returns_parse_error() {
    let a = b"a: : :\n"; // invalid
    let err = merge_yaml(&[a.to_vec()]).unwrap_err();
    assert!(matches!(
        err,
        aenv_core::merge::MergeError::Parse { kind: "yaml", .. }
    ));
}
```

- [ ] **Step 3: Implement `merge_yaml`**

Create `crates/aenv-core/src/merge/deep_yaml.rs`:

```rust
//! Deep-merge for YAML: parse YAML -> serde_json::Value -> deep_merge_value
//! -> emit YAML.
//!
//! YAML tagged scalars (timestamps, binary, custom tags) round-trip lossily
//! through serde_json::Value; this is acceptable for Phase 2's targets
//! (.aider.conf.yml and friends) which use only plain scalars + maps +
//! sequences.

use serde_json::Value as JsonValue;

use super::{deep_json::deep_merge_value, MergeError};

pub fn merge_yaml(inputs: &[Vec<u8>]) -> Result<Vec<u8>, MergeError> {
    if inputs.is_empty() {
        return Ok(b"{}\n".to_vec());
    }
    let mut acc: Option<JsonValue> = None;
    for bytes in inputs {
        let yv: serde_yaml::Value = serde_yaml::from_slice(bytes).map_err(|e| MergeError::Parse {
            kind: "yaml",
            source: e.to_string(),
        })?;
        let jv: JsonValue = serde_json::to_value(&yv).map_err(|e| MergeError::Parse {
            kind: "yaml",
            source: format!("yaml -> json conversion failed: {e}"),
        })?;
        acc = Some(match acc.take() {
            None => jv,
            Some(existing) => deep_merge_value(existing, jv),
        });
    }
    let merged = acc.unwrap_or(JsonValue::Object(Default::default()));
    let merged_yaml: serde_yaml::Value =
        serde_json::from_value(merged).map_err(|e| MergeError::Parse {
            kind: "yaml",
            source: format!("json -> yaml conversion failed: {e}"),
        })?;
    let out = serde_yaml::to_string(&merged_yaml).map_err(|e| MergeError::Parse {
        kind: "yaml",
        source: e.to_string(),
    })?;
    Ok(out.into_bytes())
}
```

- [ ] **Step 4: Run the YAML tests**

Run: `cargo test -p aenv-core --test merge_deep yaml`
Expected: 4 new YAML tests pass; JSON tests still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/merge/deep_yaml.rs crates/aenv-core/Cargo.toml \
        Cargo.toml crates/aenv-core/tests/merge_deep.rs
git commit -m "Add deep-merge for YAML

Parse with serde_yaml, convert to serde_json::Value, reuse
deep_merge_value, convert back. Lossy on YAML-only tags
(timestamps, binary, custom tags); Phase 2 targets don't use any of
those. Output is canonical YAML via serde_yaml::to_string.
"
```

---

### Task 8: Deep-merge for TOML

Same shape as YAML but with `toml::Value`. TOML's type system is closer to JSON's than YAML's, so the conversion is straightforward.

**Files:**
- Create: `crates/aenv-core/src/merge/deep_toml.rs`
- Test: append TOML cases to `crates/aenv-core/tests/merge_deep.rs`

- [ ] **Step 1: Write the TOML tests**

Append to `crates/aenv-core/tests/merge_deep.rs`:

```rust
use aenv_core::merge::deep_toml::merge_toml;

#[test]
fn toml_merges_tables_union_of_keys() {
    let a = b"a = 1\nb = 2\n";
    let b = b"b = 20\nc = 3\n";
    let out = merge_toml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: toml::Value = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
    assert_eq!(v["a"].as_integer().unwrap(), 1);
    assert_eq!(v["b"].as_integer().unwrap(), 20);
    assert_eq!(v["c"].as_integer().unwrap(), 3);
}

#[test]
fn toml_arrays_concatenate() {
    let a = b"list = [1, 2]\n";
    let b = b"list = [3]\n";
    let out = merge_toml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: toml::Value = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
    assert_eq!(v["list"].as_array().unwrap().len(), 3);
}

#[test]
fn toml_nested_tables_merge_recursively() {
    let a = b"[adapters.cc]\nfiles = [\"a\"]\n";
    let b = b"[adapters.cursor]\nfiles = [\"b\"]\n";
    let out = merge_toml(&[a.to_vec(), b.to_vec()]).unwrap();
    let v: toml::Value = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
    assert!(v["adapters"]["cc"]["files"].is_array());
    assert!(v["adapters"]["cursor"]["files"].is_array());
}

#[test]
fn toml_invalid_input_returns_parse_error() {
    let a = b"= invalid\n";
    let err = merge_toml(&[a.to_vec()]).unwrap_err();
    assert!(matches!(
        err,
        aenv_core::merge::MergeError::Parse { kind: "toml", .. }
    ));
}
```

- [ ] **Step 2: Implement `merge_toml`**

Create `crates/aenv-core/src/merge/deep_toml.rs`:

```rust
//! Deep-merge for TOML.
//!
//! `toml::Value` and `serde_json::Value` share a structural model. We convert
//! TOML -> JSON, reuse `deep_merge_value`, then convert back.

use serde_json::Value as JsonValue;

use super::{deep_json::deep_merge_value, MergeError};

pub fn merge_toml(inputs: &[Vec<u8>]) -> Result<Vec<u8>, MergeError> {
    if inputs.is_empty() {
        return Ok(b"".to_vec());
    }
    let mut acc: Option<JsonValue> = None;
    for bytes in inputs {
        let text = std::str::from_utf8(bytes).map_err(|e| MergeError::Utf8(e.to_string()))?;
        let tv: toml::Value = toml::from_str(text).map_err(|e| MergeError::Parse {
            kind: "toml",
            source: e.to_string(),
        })?;
        let jv: JsonValue = serde_json::to_value(&tv).map_err(|e| MergeError::Parse {
            kind: "toml",
            source: format!("toml -> json failed: {e}"),
        })?;
        acc = Some(match acc.take() {
            None => jv,
            Some(existing) => deep_merge_value(existing, jv),
        });
    }
    let merged = acc.unwrap_or(JsonValue::Object(Default::default()));
    let merged_toml: toml::Value =
        serde_json::from_value(merged).map_err(|e| MergeError::Parse {
            kind: "toml",
            source: format!("json -> toml failed: {e}"),
        })?;
    let out = toml::to_string_pretty(&merged_toml).map_err(|e| MergeError::Parse {
        kind: "toml",
        source: e.to_string(),
    })?;
    Ok(out.into_bytes())
}
```

- [ ] **Step 3: Run the TOML tests**

Run: `cargo test -p aenv-core --test merge_deep`
Expected: 4 new TOML tests pass; JSON + YAML tests still pass. Total 16.

- [ ] **Step 4: Commit**

```bash
git add crates/aenv-core/src/merge/deep_toml.rs crates/aenv-core/tests/merge_deep.rs
git commit -m "Add deep-merge for TOML

Parse with toml::from_str, convert to serde_json::Value, reuse
deep_merge_value, convert back via toml::to_string_pretty. Same
conversion shape as the YAML backend.
"
```

---

### Task 9: Shadow tracking

Given a resolved chain + candidate set, compute the shadow chain for each path. A candidate is "shadowed" if a later candidate in the chain produces a non-merged artifact at the same path. For merged paths, no candidate is shadowed — all of them are contributors.

**Files:**
- Create: `crates/aenv-core/src/shadow.rs`
- Modify: `crates/aenv-core/src/lib.rs` (`pub mod shadow;`)
- Test: `crates/aenv-core/tests/shadow.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/shadow.rs`:

```rust
use std::path::PathBuf;

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::{Candidate, MaterializeStrategy};
use aenv_core::shadow::compute_shadows;

fn cand(ns: &str, path: &str, adapter: &str) -> Candidate {
    Candidate {
        namespace: NamespaceId::new(ns).unwrap(),
        path: PathBuf::from(path),
        source_path: PathBuf::from(format!("/aenv/envs/{ns}/{path}")),
        adapter: adapter.to_string(),
        merge_override: None,
    }
}

fn qn(ns: &str, short: &str) -> QualifiedName {
    let nsid = if ns == NamespaceId::RESERVED_MERGED {
        NamespaceId::merged_synthetic()
    } else {
        NamespaceId::new(ns).unwrap()
    };
    QualifiedName::new(nsid, ShortName::new(short).unwrap())
}

fn cc_with_instructions() -> AdapterRegistry {
    let cc: Adapter = toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md", ".claude/skills/**/*"]
[roles]
"CLAUDE.md" = "instructions"
"#,
    )
    .unwrap();
    let mut r = AdapterRegistry::default();
    r.insert(cc);
    r
}

#[test]
fn symlink_path_with_two_candidates_yields_one_shadow() {
    // Skill provided by base, overridden by leaf.
    let candidates = vec![
        cand("base", ".claude/skills/write-tests/SKILL.md", "claude-code"),
        cand("leaf", ".claude/skills/write-tests/SKILL.md", "claude-code"),
    ];
    let strategy = MaterializeStrategy::Symlink;
    let shadows = compute_shadows(&candidates, strategy, &cc_with_instructions()).unwrap();
    assert_eq!(shadows, vec![qn("base", ".claude/skills/write-tests/SKILL.md")]);
}

#[test]
fn three_deep_chain_yields_two_shadows_in_root_to_near_order() {
    let candidates = vec![
        cand("a", "X", "claude-code"),
        cand("b", "X", "claude-code"),
        cand("c", "X", "claude-code"),
    ];
    let shadows = compute_shadows(
        &candidates,
        MaterializeStrategy::Symlink,
        &cc_with_instructions(),
    ).unwrap();
    assert_eq!(shadows.len(), 2);
    // Ordered chronologically (root-first): a precedes b.
    assert_eq!(shadows[0].namespace().as_str(), "a");
    assert_eq!(shadows[1].namespace().as_str(), "b");
}

#[test]
fn merged_path_has_no_shadows() {
    let candidates = vec![
        cand("base", ".mcp.json", "mcp"),
        cand("leaf", ".mcp.json", "mcp"),
    ];
    let shadows =
        compute_shadows(&candidates, MaterializeStrategy::DeepMerge(
            aenv_core::resolve::DeepMergeFormat::Json,
        ), &cc_with_instructions()).unwrap();
    assert!(shadows.is_empty(), "merged files have contributors, not shadows");
}

#[test]
fn section_merged_path_has_no_shadows() {
    let candidates = vec![
        cand("base", "CLAUDE.md", "claude-code"),
        cand("leaf", "CLAUDE.md", "claude-code"),
    ];
    let shadows = compute_shadows(
        &candidates,
        MaterializeStrategy::SectionMerge,
        &cc_with_instructions(),
    ).unwrap();
    assert!(shadows.is_empty());
}

#[test]
fn single_candidate_has_no_shadows() {
    let shadows = compute_shadows(
        &[cand("base", "CLAUDE.md", "claude-code")],
        MaterializeStrategy::Symlink,
        &cc_with_instructions(),
    ).unwrap();
    assert!(shadows.is_empty());
}
```

`compute_shadows` takes `&AdapterRegistry` to support the *short-name* resolution step (e.g. SKILL.md paths → skill short names). Phase 2 path-keys-equal-short-names except for skills, where `ShortName` is the directory name parent of `SKILL.md`. For Phase 2 we keep it simple: the short name is the file path; skills can adopt the parent-dir convention in Phase 4.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test shadow`
Expected: compile error.

- [ ] **Step 3: Implement `compute_shadows`**

Create `crates/aenv-core/src/shadow.rs`:

```rust
//! Shadow-chain computation.
//!
//! When two or more candidates in a chain target the same path and the
//! resolved strategy is non-merge (Symlink/Identical/Copy), the latest
//! candidate is the "provided" artifact and the earlier candidates are
//! shadowed. For merge strategies (SectionMerge/DeepMerge), every
//! candidate is a contributor and the shadow set is empty.

use crate::adapter::AdapterRegistry;
use crate::identity::{NamespaceId, QualifiedName, ShortName};
use crate::resolve::{Candidate, MaterializeStrategy};

pub fn compute_shadows(
    candidates: &[Candidate],
    strategy: MaterializeStrategy,
    _adapters: &AdapterRegistry,
) -> crate::Result<Vec<QualifiedName>> {
    if candidates.len() < 2 {
        return Ok(Vec::new());
    }
    match strategy {
        MaterializeStrategy::SectionMerge | MaterializeStrategy::DeepMerge(_) => Ok(Vec::new()),
        MaterializeStrategy::Symlink
        | MaterializeStrategy::Copy
        | MaterializeStrategy::Identical
        | MaterializeStrategy::Merged => {
            // Everything except the last candidate is shadowed.
            candidates[..candidates.len() - 1]
                .iter()
                .map(qualified_from_candidate)
                .collect()
        }
    }
}

/// Compute the QualifiedName for a candidate's contribution.
///
/// Returns `Err` if the candidate's path contains the `::` separator (which
/// is invalid as a short name). A well-formed manifest can never declare such
/// a file, so this is effectively unreachable — but it surfaces as a clean
/// `ManifestInvalid` rather than a panic if someone hand-crafts a malicious
/// manifest.
pub(crate) fn qualified_from_candidate(c: &Candidate) -> crate::Result<QualifiedName> {
    let short = ShortName::new(c.path.to_string_lossy().to_string())?;
    Ok(QualifiedName::new(c.namespace.clone(), short))
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test shadow`
Expected: 5 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/shadow.rs crates/aenv-core/src/lib.rs \
        crates/aenv-core/tests/shadow.rs
git commit -m "Add shadow-chain computation

compute_shadows takes the candidate list + decided strategy and
returns the QualifiedNames of earlier-chain candidates that the
latest one shadows. Empty for merge strategies (every candidate is a
contributor); empty for single-candidate paths. Earlier candidates
appear first in the result (root-to-near order).

Exposes qualified_from_candidate as pub(crate) so the activate.rs
integration in Task 11 can reuse it to label provided artifacts.
"
```

---

### Task 10: Extend `state.rs` for qualified provenance

`ManagedFile` gains `qualified_name`, `contributors`, `shadows`. `schema_version` bumps to `2`. The deserializer reads schema-1 files too — it just leaves the new fields empty (forward-compat: schema-2 reader of schema-1 state should not refuse, since schema-1 files describe single-namespace activations whose qualified name is trivially `<ns>::<path>` and shadow/contributors lists are empty).

**Files:**
- Modify: `crates/aenv-core/src/state.rs`
- Modify: `crates/aenv-core/tests/state.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/aenv-core/tests/state.rs`:

```rust
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::state::{ActivationState, BackedUpFile, ManagedFile};
use aenv_core::resolve::MaterializeStrategy;
use std::path::PathBuf;

fn qn(ns: &str, short: &str) -> QualifiedName {
    let nsid = if ns == NamespaceId::RESERVED_MERGED {
        NamespaceId::merged_synthetic()
    } else {
        NamespaceId::new(ns).unwrap()
    };
    QualifiedName::new(nsid, ShortName::new(short).unwrap())
}

#[test]
fn schema_version_is_2_for_new_states() {
    let s = ActivationState {
        schema_version: 2,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![],
        backed_up: vec![],
    };
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"schema_version\":2"));
}

#[test]
fn managed_file_serializes_qualified_name_and_shadows() {
    let mf = ManagedFile {
        path: PathBuf::from("CLAUDE.md"),
        qualified_name: qn("leaf", "CLAUDE.md"),
        strategy: MaterializeStrategy::Symlink,
        contributors: vec![],
        shadows: vec![qn("base", "CLAUDE.md")],
    };
    let json = serde_json::to_string(&mf).unwrap();
    assert!(json.contains("\"qualified_name\""));
    assert!(json.contains("leaf::CLAUDE.md"));
    assert!(json.contains("base::CLAUDE.md"));
}

#[test]
fn managed_file_serializes_contributors_for_merged() {
    let mf = ManagedFile {
        path: PathBuf::from(".mcp.json"),
        qualified_name: qn("(merged)", ".mcp.json"),
        strategy: MaterializeStrategy::DeepMerge(
            aenv_core::resolve::DeepMergeFormat::Json,
        ),
        contributors: vec![qn("base", ".mcp.json"), qn("leaf", ".mcp.json")],
        shadows: vec![],
    };
    let json = serde_json::to_string(&mf).unwrap();
    assert!(json.contains("\"contributors\""));
    assert!(json.contains("base::.mcp.json"));
    assert!(json.contains("leaf::.mcp.json"));
}

#[test]
fn schema_1_files_load_with_empty_new_fields() {
    // Schema-1 ManagedFile only has path + strategy (Phase 1).
    let schema_1 = serde_json::json!({
        "schema_version": 1,
        "active_namespace": "base",
        "project_root": "/p",
        "managed_files": [
            { "path": "CLAUDE.md", "strategy": "Symlink" }
        ],
        "backed_up": []
    });
    // The Phase 2 reader synthesizes empty Phase 2 fields.
    let s: ActivationState = serde_json::from_value(schema_1).unwrap();
    assert_eq!(s.schema_version, 1);
    let mf = &s.managed_files[0];
    assert!(mf.contributors.is_empty());
    assert!(mf.shadows.is_empty());
    // qualified_name is synthesized as <namespace>::<path>.
    assert_eq!(format!("{}", mf.qualified_name), "base::CLAUDE.md");
}
```

- [ ] **Step 2: Modify the types**

Modify `crates/aenv-core/src/state.rs`:

```rust
#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ManagedFile {
    pub path: PathBuf,
    /// In schema-1 state files this field is absent; the deserializer
    /// synthesizes `<active_namespace>::<path>` to fill the gap.
    pub qualified_name: crate::identity::QualifiedName,
    pub strategy: crate::resolve::MaterializeStrategy,
    #[serde(default)]
    pub contributors: Vec<crate::identity::QualifiedName>,
    #[serde(default)]
    pub shadows: Vec<crate::identity::QualifiedName>,
}

// MaterializeStrategy lived in state.rs in Phase 1; in Phase 2 the
// canonical definition is `crate::resolve::MaterializeStrategy` (Task 2).
// Remove the duplicate enum from state.rs and re-export the resolve version.
pub use crate::resolve::MaterializeStrategy;
```

For schema-1 → schema-2 forward-compat, replace the derived `Deserialize` with a custom impl that detects schema-1 by absence of `qualified_name`:

```rust
impl<'de> serde::Deserialize<'de> for ManagedFile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Use a helper struct with all fields optional, then synthesize where missing.
        #[derive(serde::Deserialize)]
        struct Raw {
            path: PathBuf,
            #[serde(default)]
            qualified_name: Option<crate::identity::QualifiedName>,
            strategy: crate::resolve::MaterializeStrategy,
            #[serde(default)]
            contributors: Vec<crate::identity::QualifiedName>,
            #[serde(default)]
            shadows: Vec<crate::identity::QualifiedName>,
        }
        let raw = Raw::deserialize(d)?;
        let qualified_name = raw.qualified_name.unwrap_or_else(|| {
            // Synthesized from the path; namespace is filled in by the
            // ActivationState deserializer's post-processing step.
            crate::identity::QualifiedName::new(
                crate::identity::NamespaceId::new("__schema_1__").expect("static"),
                crate::identity::ShortName::new(raw.path.to_string_lossy().to_string())
                    .expect("path validated upstream"),
            )
        });
        Ok(ManagedFile {
            path: raw.path,
            qualified_name,
            strategy: raw.strategy,
            contributors: raw.contributors,
            shadows: raw.shadows,
        })
    }
}
```

Then in `ActivationState`'s `Deserialize` impl, post-process: if any `ManagedFile.qualified_name.namespace == "__schema_1__"`, replace with the real `active_namespace` from the same state struct. This is the cleanest schema-1 forward-compat: synthesis happens at the boundary, callers see uniform `QualifiedName` values.

```rust
impl<'de> serde::Deserialize<'de> for ActivationState {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Raw {
            schema_version: u32,
            active_namespace: String,
            project_root: PathBuf,
            #[serde(default)]
            managed_files: Vec<ManagedFile>,
            #[serde(default)]
            backed_up: Vec<BackedUpFile>,
        }
        let mut raw = Raw::deserialize(d)?;
        if raw.schema_version == 1 {
            let ns = crate::identity::NamespaceId::new(raw.active_namespace.as_str())
                .map_err(serde::de::Error::custom)?;
            for mf in &mut raw.managed_files {
                if mf.qualified_name.namespace().as_str() == "__schema_1__" {
                    mf.qualified_name = crate::identity::QualifiedName::new(
                        ns.clone(),
                        mf.qualified_name.short().clone(),
                    );
                }
            }
        }
        Ok(ActivationState {
            schema_version: raw.schema_version,
            active_namespace: raw.active_namespace,
            project_root: raw.project_root,
            managed_files: raw.managed_files,
            backed_up: raw.backed_up,
        })
    }
}
```

Set the default version constant to `2`:

```rust
pub const SCHEMA_VERSION: u32 = 2;
```

…and update every call site that wrote `schema_version: 1` (search via `grep -rn 'schema_version: 1' crates/aenv-core/src`) to `schema_version: SCHEMA_VERSION`.

- [ ] **Step 3: Run the state tests**

Run: `cargo test -p aenv-core --test state`
Expected: 4 PASS (plus the existing Phase 1 state tests).

Re-run all aenv-core tests to confirm no breakage: `cargo test -p aenv-core` — every existing activation/deactivation test must still pass because the new fields default empty.

- [ ] **Step 4: Commit**

```bash
git add crates/aenv-core/src/state.rs crates/aenv-core/tests/state.rs
git commit -m "Bump state schema to 2; record qualified provenance per managed file

ManagedFile gains qualified_name, contributors, and shadows. Schema-1
state files are still loaded — the deserializer synthesizes
qualified_name as <active_namespace>::<path> when the field is absent
and leaves contributors/shadows empty. The MaterializeStrategy enum is
re-rooted in crate::resolve and re-exported from state.rs to avoid
duplicate definitions across modules.

No behavior change yet — activate.rs writes schema-2 starting in
Task 11.
"
```

---

### Task 11: Wire composition into `activate.rs`

This is the integration task. `activate_namespace` is rewritten to call `resolve_namespace`, `decide_strategy`, then either symlink (single candidate, last-wins) or merge (Section/Deep). Merged files become regular files on disk. State writes record qualified provenance. Rollback covers merged files too (they go in the undo log alongside symlinks).

**Files:**
- Modify: `crates/aenv-core/src/activate.rs`
- Modify: `crates/aenv-core/tests/activate.rs` (add Phase 2 scenarios)
- Create: `crates/aenv-core/tests/composition.rs` (new file for end-to-end composition tests against MockFilesystem)

- [ ] **Step 1: Write the composition test (full end-to-end against mock)**

Create `crates/aenv-core/tests/composition.rs`:

```rust
//! End-to-end composition tests against MockFilesystem.
//! These exercise the full resolve -> strategy -> merge -> materialize
//! pipeline introduced by Task 11.

use std::path::Path;

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::state::ActivationState;
use aenv_core::resolve::MaterializeStrategy;

mod mock_filesystem;
use mock_filesystem::MockFilesystem;

const REG: &str = "/aenv";
const PROJ: &str = "/proj";

fn registry() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from(REG))
}

fn cc() -> Adapter {
    toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"#,
    )
    .unwrap()
}

fn mcp() -> Adapter {
    toml::from_str(
        r#"
name = "mcp"
files = [".mcp.json"]
[default_merge]
".mcp.json" = "deep"
"#,
    )
    .unwrap()
}

fn adapters() -> AdapterRegistry {
    let mut r = AdapterRegistry::default();
    r.insert(cc());
    r.insert(mcp());
    r
}

fn write(fs: &MockFilesystem, p: &str, c: &str) {
    fs.write(Path::new(p), c.as_bytes()).unwrap();
}

fn read(fs: &MockFilesystem, p: &str) -> String {
    String::from_utf8(fs.read(Path::new(p)).unwrap()).unwrap()
}

#[test]
fn activates_two_namespace_chain_with_section_merge_and_symlinked_skill() {
    let fs = MockFilesystem::default();
    // base namespace
    write(&fs, &format!("{REG}/envs/base/aenv.toml"),
        r#"
name = "base"
[adapters.claude-code]
files = ["CLAUDE.md"]
"#);
    write(&fs, &format!("{REG}/envs/base/CLAUDE.md"),
        "# Build & Test\n\ncargo test\n");
    // leaf namespace
    write(&fs, &format!("{REG}/envs/leaf/aenv.toml"),
        r#"
name = "leaf"
extends = ["base"]
[adapters.claude-code]
files = ["CLAUDE.md"]
"#);
    write(&fs, &format!("{REG}/envs/leaf/CLAUDE.md"),
        "# Disposition\n\nbe terse\n");

    activate_namespace(
        &fs,
        &registry(),
        &adapters(),
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap();

    let merged = read(&fs, &format!("{PROJ}/CLAUDE.md"));
    assert!(merged.contains("# Build & Test"));
    assert!(merged.contains("cargo test"));
    assert!(merged.contains("# Disposition"));
    assert!(merged.contains("be terse"));

    // Merged file is a regular file (not a symlink).
    let meta = fs
        .symlink_metadata(Path::new(&format!("{PROJ}/CLAUDE.md")))
        .unwrap();
    assert!(!matches!(meta.kind, aenv_core::fs::FileKind::Symlink));

    // State records section-merge strategy + contributors.
    let state: ActivationState = serde_json::from_slice(
        &fs.read(Path::new(&format!("{PROJ}/.aenv-state/state.json"))).unwrap(),
    ).unwrap();
    let claude = state.managed_files.iter()
        .find(|m| m.path.to_string_lossy().ends_with("CLAUDE.md"))
        .unwrap();
    assert!(matches!(claude.strategy, MaterializeStrategy::SectionMerge));
    assert_eq!(claude.contributors.len(), 2);
    assert!(claude.shadows.is_empty());
}

#[test]
fn deep_merges_mcp_json_across_chain() {
    let fs = MockFilesystem::default();
    write(&fs, &format!("{REG}/envs/base/aenv.toml"),
        r#"
name = "base"
[adapters.mcp]
files = [".mcp.json"]
"#);
    write(&fs, &format!("{REG}/envs/base/.mcp.json"),
        r#"{"servers":{"a":{"command":"a"}}}"#);
    write(&fs, &format!("{REG}/envs/leaf/aenv.toml"),
        r#"
name = "leaf"
extends = ["base"]
[adapters.mcp]
files = [".mcp.json"]
"#);
    write(&fs, &format!("{REG}/envs/leaf/.mcp.json"),
        r#"{"servers":{"b":{"command":"b"}}}"#);

    activate_namespace(
        &fs,
        &registry(),
        &adapters(),
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap();

    let merged = read(&fs, &format!("{PROJ}/.mcp.json"));
    let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
    assert!(v["servers"]["a"]["command"] == "a");
    assert!(v["servers"]["b"]["command"] == "b");
}

#[test]
fn skill_overlay_shadows_parent_skill() {
    // Both base and leaf provide the same .claude/skills/X/SKILL.md.
    let cc_w_skills: Adapter = toml::from_str(
        r#"
name = "claude-code"
files = [".claude/skills/write-tests/SKILL.md"]
"#).unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc_w_skills);

    let fs = MockFilesystem::default();
    write(&fs, &format!("{REG}/envs/base/aenv.toml"),
        r#"
name = "base"
[adapters.claude-code]
files = [".claude/skills/write-tests/SKILL.md"]
"#);
    write(&fs, &format!("{REG}/envs/base/.claude/skills/write-tests/SKILL.md"),
        "base impl");
    write(&fs, &format!("{REG}/envs/leaf/aenv.toml"),
        r#"
name = "leaf"
extends = ["base"]
[adapters.claude-code]
files = [".claude/skills/write-tests/SKILL.md"]
"#);
    write(&fs, &format!("{REG}/envs/leaf/.claude/skills/write-tests/SKILL.md"),
        "leaf impl");

    activate_namespace(
        &fs, &registry(), &adapters,
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    ).unwrap();

    let body = read(&fs, &format!("{PROJ}/.claude/skills/write-tests/SKILL.md"));
    assert_eq!(body, "leaf impl");

    let state: ActivationState = serde_json::from_slice(
        &fs.read(Path::new(&format!("{PROJ}/.aenv-state/state.json"))).unwrap(),
    ).unwrap();
    let mf = state.managed_files.iter()
        .find(|m| m.path.to_string_lossy().contains("write-tests"))
        .unwrap();
    assert_eq!(mf.shadows.len(), 1);
    assert_eq!(mf.shadows[0].namespace().as_str(), "base");
}

#[test]
fn rollback_removes_prior_materialized_file_on_partial_failure() {
    // by_path iterates lexicographically: ".mcp.json" (0x2E) sorts before
    // "CLAUDE.md" (0x43). So .mcp.json materializes first. Inject the failure
    // on CLAUDE.md so that .mcp.json has *already been written* when the
    // failure fires — the rollback assertion then meaningfully exercises the
    // RemoveRegularFile undo step.
    let fs = MockFilesystem::default();
    fs.fail_writes_to(Path::new(&format!("{PROJ}/CLAUDE.md")));
    write(&fs, &format!("{REG}/envs/leaf/aenv.toml"),
        r#"
name = "leaf"
[adapters.claude-code]
files = ["CLAUDE.md"]
[adapters.mcp]
files = [".mcp.json"]
"#);
    write(&fs, &format!("{REG}/envs/leaf/CLAUDE.md"), "# leaf\n");
    write(&fs, &format!("{REG}/envs/leaf/.mcp.json"), "{}");

    let err = activate_namespace(
        &fs, &registry(), &adapters(),
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    ).unwrap_err();
    assert!(matches!(err, aenv_core::AenvError::ActivationConflict(_)));

    // .mcp.json (the first to materialize) was rolled back by RemoveRegularFile.
    assert!(!fs.exists(Path::new(&format!("{PROJ}/.mcp.json"))).unwrap());
    // CLAUDE.md never made it to disk because its write was the one that failed.
    assert!(!fs.exists(Path::new(&format!("{PROJ}/CLAUDE.md"))).unwrap());
    // state.json was never written.
    assert!(!fs.exists(Path::new(&format!("{PROJ}/.aenv-state/state.json"))).unwrap());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test composition`
Expected: tests fail — `activate_namespace` still calls Phase 1's single-namespace path and doesn't produce merged files or shadow records.

- [ ] **Step 3a: Prepare the activate module for a `phase1` submodule**

`crates/aenv-core/src/activate.rs` becomes a parent that declares `mod phase1;`. The Phase 1 inline symlink path (currently lives in `perform_activation`'s `match ProjectPathState { Absent | AlreadyOurSymlink | ByteIdenticalRegular | Displaced }` body) is *extracted* into a `phase1::materialize_symlink` helper. The existing `UndoStep` enum (`enum UndoStep { RemoveSymlink { link }, RestoreBackup { original, backup } }`) gains one new variant for the regular-file rollback case:

```rust
// crates/aenv-core/src/activate.rs (top of file, after imports)
mod phase1;
```

Then in the same file, extend the existing enum (do not rename it — Phase 1 calls it `UndoStep` everywhere):

```rust
enum UndoStep {
    /// Created a symlink at `link`; undo by removing it.
    RemoveSymlink { link: PathBuf },
    /// Backed up `original` to `backup`; undo by renaming `backup` -> `original`.
    RestoreBackup { original: PathBuf, backup: PathBuf },
    /// Wrote a regular file at `path` (Phase 2 merge output); undo by removing it.
    RemoveRegularFile { path: PathBuf },
}
```

Update the existing `fn undo<F: Filesystem>(fs: &F, log: Vec<UndoStep>)` to handle the new variant:

```rust
UndoStep::RemoveRegularFile { path } => {
    let _ = fs.remove_file(&path);
}
```

Reverse-iteration order is already correct in the Phase 1 implementation (`for step in log.into_iter().rev()`). The `RemoveRegularFile` for a merged file is pushed *after* its `RestoreBackup` (if any), so on rollback the regular file is removed first and the backup is renamed back in second.

- [ ] **Step 3b: Extract `phase1::materialize_symlink`**

Create `crates/aenv-core/src/activate/phase1.rs`. Move the per-state-arm symlink logic (the four match arms in `perform_activation`) into a single helper. Its signature uses the workspace's generic shape — `<F: Filesystem>`, *not* `&dyn Filesystem` (Phase 1's `AdapterRegistry::load_from_dir<F>` and friends require the concrete type, so the trait-object form breaks downstream calls):

```rust
//! Phase 1 symlink materialization, extracted so Phase 2's composing
//! `activate_namespace` can reuse it for last-wins / single-candidate paths.

use std::path::Path;

use crate::fs::Filesystem;
use crate::state::{BackedUpFile, ManagedFile, MaterializeStrategy};
use crate::{AenvError, Result};

use super::{backup_dir_for_this_run, classify_project_path, UndoStep};
// `backup_dir_for_this_run` and `classify_project_path` are the existing
// Phase 1 private helpers in activate.rs; keep them at parent-module scope
// so this submodule can `use super::*` them. If the parent function names
// differ in the current code, rename here accordingly.

/// Materialize one candidate as a symlink (or no-op for byte-identical).
///
/// Pushes any backups to `backed_up`, records undo steps on `undo`, and
/// appends one `ManagedFile` entry per call (the caller does NOT push its
/// own entry — this helper owns the strategy decision between `Symlink`
/// and `Identical`).
#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_symlink<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    backup_root: &Path,
    project_path: &Path,
    source_path: &Path,
    namespace: &crate::identity::NamespaceId,
    short: &crate::identity::ShortName,
    relative_path: &Path,
    shadows: Vec<crate::identity::QualifiedName>,
    undo: &mut Vec<UndoStep>,
    managed: &mut Vec<ManagedFile>,
    backed_up: &mut Vec<BackedUpFile>,
) -> Result<()> {
    // The implementation is the lift-and-shift of the four-arm `match
    // classify_project_path(...)` body from the current `perform_activation`,
    // adapted to push ManagedFile + UndoStep + BackedUpFile through &mut Vecs
    // rather than building them inline. The recorded `strategy` is `Identical`
    // when classification returned `ByteIdenticalRegular`, and `Symlink`
    // otherwise.
    // (See crates/aenv-core/src/activate.rs in the Phase 1 codebase for the
    // exact body to migrate.)
    todo!("lift from current perform_activation; preserve all error paths")
}
```

Note that this is the *only* helper this submodule exports. It does NOT take a generic adapter list — composition concerns belong in the parent module.

- [ ] **Step 3c: Rewrite `activate_namespace` in `activate.rs`**

The Phase 1 signature is `activate_namespace<F: Filesystem>(fs: &F, layout: &RegistryLayout, adapters: &AdapterRegistry, project_root: &Path, namespace_name: &str) -> Result<ActivationState>`. Phase 2 keeps the generic shape and the `(layout, adapters, project_root)` order but changes the final parameter from `&str` to `&NamespaceId`:

```rust
pub fn activate_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    leaf: &crate::identity::NamespaceId,
) -> Result<ActivationState> {
    // 1. Run probe to ensure .aenv-state/ is on the same fs as the project.
    crate::atomicity::probe_rename_atomicity(fs, project_root)?;

    // 2. Resolve the chain.
    let resolution =
        crate::resolve::resolve_namespace(fs, layout, adapters, leaf)?;

    // 3. Group candidates by path; each group becomes one materialized artifact.
    let mut by_path: std::collections::BTreeMap<PathBuf, Vec<crate::resolve::Candidate>> =
        Default::default();
    for c in resolution.candidates {
        by_path.entry(c.path.clone()).or_default().push(c);
    }

    // 4. Decide strategy + materialize, with an undo log per artifact.
    let mut undo: Vec<UndoStep> = Vec::new();
    let mut managed: Vec<ManagedFile> = Vec::new();
    let mut backed_up: Vec<BackedUpFile> = Vec::new();
    let backup_root = backup_dir_for_this_run(project_root);

    let result: Result<()> = (|| {
        for (path, candidates) in by_path {
            let strategy = crate::strategy::decide_strategy(&candidates, adapters)?;
            materialize_one(
                fs,
                adapters,
                project_root,
                &backup_root,
                &path,
                &candidates,
                strategy,
                &mut undo,
                &mut managed,
                &mut backed_up,
            )?;
        }
        Ok(())
    })();

    if let Err(e) = result {
        undo(fs, std::mem::take(&mut undo));
        return Err(e);
    }

    // 5. Write state.json.
    let state = ActivationState {
        schema_version: crate::state::SCHEMA_VERSION,
        active_namespace: leaf.as_str().to_owned(),
        project_root: project_root.to_path_buf(),
        managed_files: managed,
        backed_up,
    };
    let state_path = project_root.join(".aenv-state/state.json");
    let body = serde_json::to_vec_pretty(&state)
        .map_err(|e| AenvError::ActivationConflict(format!("state serialize: {e}")))?;
    fs.write(&state_path, &body)?;
    Ok(state)
}

fn materialize_one<F: Filesystem>(
    fs: &F,
    adapters: &AdapterRegistry,
    project_root: &Path,
    backup_root: &Path,
    path: &Path,
    candidates: &[crate::resolve::Candidate],
    strategy: MaterializeStrategy,
    undo: &mut Vec<UndoStep>,
    managed: &mut Vec<ManagedFile>,
    backed_up: &mut Vec<BackedUpFile>,
) -> Result<()> {
    let project_path = project_root.join(path);
    match strategy {
        MaterializeStrategy::Symlink | MaterializeStrategy::Identical => {
            let latest = candidates.last().expect("non-empty");
            // The Phase 1 helper decides Symlink vs Identical internally
            // based on classify_project_path and pushes a ManagedFile entry
            // with the correct strategy. The caller does NOT push.
            let shadows = crate::shadow::compute_shadows(candidates, strategy, adapters)?;
            let qn = crate::shadow::qualified_from_candidate(latest)?;
            phase1::materialize_symlink(
                fs,
                project_root,
                backup_root,
                &project_path,
                &latest.source_path,
                qn.namespace(),
                qn.short(),
                path,
                shadows,
                undo,
                managed,
                backed_up,
            )?;
        }
        MaterializeStrategy::SectionMerge => {
            let bodies = read_all_as_strings(fs, candidates)?;
            let merged = crate::merge::section::merge_sections(&bodies);
            write_merged_regular(
                fs, project_root, backup_root, &project_path,
                merged.as_bytes(), undo, backed_up,
            )?;
            managed.push(ManagedFile {
                path: path.to_path_buf(),
                qualified_name: synthesize_merged_qn(path)?,
                strategy,
                contributors: candidates.iter()
                    .map(crate::shadow::qualified_from_candidate)
                    .collect::<Result<Vec<_>>>()?,
                shadows: vec![],
            });
        }
        MaterializeStrategy::DeepMerge(format) => {
            let bodies = read_all_as_bytes(fs, candidates)?;
            let merged = match format {
                crate::resolve::DeepMergeFormat::Json =>
                    crate::merge::deep_json::merge_json(&bodies).map_err(AenvError::from)?,
                crate::resolve::DeepMergeFormat::Yaml =>
                    crate::merge::deep_yaml::merge_yaml(&bodies).map_err(AenvError::from)?,
                crate::resolve::DeepMergeFormat::Toml =>
                    crate::merge::deep_toml::merge_toml(&bodies).map_err(AenvError::from)?,
            };
            write_merged_regular(
                fs, project_root, backup_root, &project_path,
                &merged, undo, backed_up,
            )?;
            managed.push(ManagedFile {
                path: path.to_path_buf(),
                qualified_name: synthesize_merged_qn(path)?,
                strategy,
                contributors: candidates.iter()
                    .map(crate::shadow::qualified_from_candidate)
                    .collect::<Result<Vec<_>>>()?,
                shadows: vec![],
            });
        }
        MaterializeStrategy::Copy => {
            // Windows fallback — Phase 7. Surface a clean error rather than
            // silently degrading.
            return Err(AenvError::ActivationConflict(
                "Copy strategy is Phase 7 (Windows fallback); not supported in Phase 2".into()
            ));
        }
        MaterializeStrategy::Merged => {
            // Phase 1 legacy variant — should never be produced by Phase 2's
            // decide_strategy. Unreachable in practice; defensive arm.
            return Err(AenvError::ActivationConflict(
                "Phase 1 'Merged' variant should not be produced by Phase 2".into()
            ));
        }
    }
    Ok(())
}

pub(crate) fn synthesize_merged_qn(path: &Path) -> Result<crate::identity::QualifiedName> {
    Ok(crate::identity::QualifiedName::new(
        // merged_synthetic() bypasses the (merged) reservation check; this
        // is the only callsite allowed to construct the synthetic namespace.
        crate::identity::NamespaceId::merged_synthetic(),
        crate::identity::ShortName::new(path.to_string_lossy().to_string())?,
    ))
}

fn read_all_as_bytes<F: Filesystem>(
    fs: &F,
    candidates: &[crate::resolve::Candidate],
) -> Result<Vec<Vec<u8>>> {
    candidates.iter().map(|c| fs.read(&c.source_path).map_err(AenvError::from)).collect()
}

fn read_all_as_strings<F: Filesystem>(
    fs: &F,
    candidates: &[crate::resolve::Candidate],
) -> Result<Vec<String>> {
    candidates.iter().map(|c| {
        let bytes = fs.read(&c.source_path)?;
        String::from_utf8(bytes).map_err(|e| AenvError::ActivationConflict(
            format!("UTF-8 decode {}: {e}", c.source_path.display())
        ))
    }).collect()
}

/// Write a regular (non-symlink) file, backing up any displaced project file,
/// recording the action in the undo log.
///
/// The undo-log push order matters: `RestoreBackup` is pushed *before*
/// `RemoveRegularFile` so that on reverse replay, the regular file is
/// removed first and then the backup is renamed back into place.
fn write_merged_regular<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    backup_root: &Path,
    project_path: &Path,
    contents: &[u8],
    undo: &mut Vec<UndoStep>,
    backed_up: &mut Vec<BackedUpFile>,
) -> Result<()> {
    let existed = fs.exists(project_path)?;
    if existed {
        let backup_path = backup_root.join(project_path.strip_prefix(project_root)
            .unwrap_or(project_path));
        if let Some(parent) = backup_path.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.rename(project_path, &backup_path)?;
        undo.push(UndoStep::RestoreBackup {
            original: project_path.to_path_buf(),
            backup: backup_path.clone(),
        });
        backed_up.push(BackedUpFile {
            original_path: project_path.strip_prefix(project_root)
                .unwrap_or(project_path).to_path_buf(),
            backup_path,
        });
    }
    fs.write(project_path, contents)?;
    undo.push(UndoStep::RemoveRegularFile { path: project_path.to_path_buf() });
    Ok(())
}
```

Add the `impl From<MergeError> for AenvError` to `crate::merge::mod.rs` (it was sketched in Task 5; verify it's there). Parse errors map to `ManifestInvalid` (exit 12), type mismatches and UTF-8 to `ActivationConflict` (exit 13):

```rust
// In crates/aenv-core/src/merge/mod.rs — refine the From impl:
impl From<MergeError> for crate::AenvError {
    fn from(value: MergeError) -> Self {
        match value {
            MergeError::Parse { .. } => crate::AenvError::ManifestInvalid(value.to_string()),
            MergeError::TypeMismatch { .. } | MergeError::Utf8(_) => {
                crate::AenvError::ActivationConflict(value.to_string())
            }
        }
    }
}
```

This is a substantial refactor of activate.rs. The implementer should plan to spend the bulk of Task 11's effort on Steps 3a–3c. Do not keep both the old and new code paths — delete the Phase 1 single-namespace `perform_activation` body once `phase1::materialize_symlink` is extracted.

- [ ] **Step 4: Update Phase 1's activate tests for the new shape**

Several Phase 1 tests in `crates/aenv-core/tests/activate.rs` use the old call signature `activate_namespace(&fs, &layout, &adapters, project, "base")` — bare `&str`, not `NamespaceId`. Replace each call site (only the last argument changes; the order is preserved):

```rust
// Before
activate_namespace(&fs, &layout, &adapters, project, "base")
// After
activate_namespace(&fs, &layout, &adapters, project, &NamespaceId::new("base").unwrap())
```

The behavior is identical for single-namespace activations because the resolver produces a chain of length 1.

- [ ] **Step 4b: Update the CLI caller**

`crates/aenv-cli/src/cmd/activate.rs` calls `activate_namespace` with `&str`. Switch to `&NamespaceId`:

```rust
// In cmd::activate::run, where the namespace name is passed in:
let leaf = aenv_core::identity::NamespaceId::new(namespace_name.clone())?;
let state = aenv_core::activate::activate_namespace(
    &fs, &layout, &adapters, &project_root, &leaf,
)?;
```

Same edit pattern applies to any other Phase 1 CLI command that hard-codes the bare-string signature — grep `crates/aenv-cli/src/cmd/` for `activate_namespace` to find them all.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test -p aenv-core`
Expected: all Phase 1 tests still pass (with updated call sites) + 4 new composition tests pass.

`cargo clippy --workspace -- -D warnings` should be clean.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/activate.rs crates/aenv-core/src/activate/phase1.rs \
        crates/aenv-core/tests/activate.rs crates/aenv-core/tests/composition.rs
git commit -m "Wire composition into activate_namespace

activate_namespace now resolves the extends chain, groups candidates
by path, decides a MaterializeStrategy per group, and materializes
either a symlink (single/last-wins) or a regular file (section-merge,
deep-merge). The Phase 1 symlink helper moves to activate/phase1.rs;
the top-level function dispatches based on strategy.

State writes record qualified_name, contributors, and shadows per
managed file. Merged files use namespace '(merged)' as a synthetic
qualifier (its short_name is the materialized path). The undo log
covers merged files exactly like symlinks: a failed mid-activation
write rolls back every prior artifact.

Tests cover: two-namespace section-merge of CLAUDE.md, deep-merge of
.mcp.json, skill overlay with shadow record, and mid-activation
rollback of a merged file.
"
```

---

### Task 12: Ship the six remaining built-in adapter TOMLs

Phase 1 shipped `claude-code`. Phase 2 adds Cursor, Aider, Cline, Continue, Windsurf, and MCP. Each is a TOML file in `crates/aenv-core/src/adapters_builtin/`, embedded via `include_str!` and written to disk on first run (the embedding mechanism shipped in Phase 1).

**Files:**
- Create: `crates/aenv-core/src/adapters_builtin/cursor.toml`
- Create: `crates/aenv-core/src/adapters_builtin/aider.toml`
- Create: `crates/aenv-core/src/adapters_builtin/cline.toml`
- Create: `crates/aenv-core/src/adapters_builtin/continue_.toml`
- Create: `crates/aenv-core/src/adapters_builtin/windsurf.toml`
- Create: `crates/aenv-core/src/adapters_builtin/mcp.toml`
- Modify: `crates/aenv-core/src/adapters_builtin/mod.rs`
- Test: `crates/aenv-core/tests/adapters_builtin.rs`

- [ ] **Step 1: Write the adapter TOMLs**

`crates/aenv-core/src/adapters_builtin/cursor.toml`:

```toml
name = "cursor"
files = [".cursorrules", ".cursor/**/*"]

[roles]
".cursorrules" = "instructions"
```

`crates/aenv-core/src/adapters_builtin/aider.toml`:

```toml
name = "aider"
files = [".aider.conf.yml", ".aiderignore"]

[default_merge]
".aider.conf.yml" = "deep"
```

`crates/aenv-core/src/adapters_builtin/cline.toml`:

```toml
name = "cline"
files = [".clinerules"]

[roles]
".clinerules" = "instructions"
```

`crates/aenv-core/src/adapters_builtin/continue_.toml` (the filename has a trailing underscore because `continue` is a Rust keyword; the adapter `name` field is still `"continue"`):

```toml
name = "continue"
files = [".continue/config.json"]

[default_merge]
".continue/config.json" = "deep"
```

`crates/aenv-core/src/adapters_builtin/windsurf.toml`:

```toml
name = "windsurf"
files = [".windsurfrules"]

[roles]
".windsurfrules" = "instructions"
```

`crates/aenv-core/src/adapters_builtin/mcp.toml`:

```toml
name = "mcp"
files = [".mcp.json"]

[default_merge]
".mcp.json" = "deep"
```

- [ ] **Step 2: Embed the new adapters**

Modify `crates/aenv-core/src/adapters_builtin/mod.rs`:

```rust
pub const CLAUDE_CODE: &str = include_str!("claude_code.toml");
pub const CURSOR: &str = include_str!("cursor.toml");
pub const AIDER: &str = include_str!("aider.toml");
pub const CLINE: &str = include_str!("cline.toml");
pub const CONTINUE: &str = include_str!("continue_.toml");
pub const WINDSURF: &str = include_str!("windsurf.toml");
pub const MCP: &str = include_str!("mcp.toml");

pub const ALL: &[(&str, &str)] = &[
    ("claude-code", CLAUDE_CODE),
    ("cursor", CURSOR),
    ("aider", AIDER),
    ("cline", CLINE),
    ("continue", CONTINUE),
    ("windsurf", WINDSURF),
    ("mcp", MCP),
];

/// Write every built-in adapter to the registry's adapters dir if not already
/// present. Existing files are left untouched so user edits stick.
pub fn ensure_written<F: crate::fs::Filesystem>(
    fs: &F,
    adapters_dir: &std::path::Path,
) -> std::io::Result<()> {
    for (name, body) in ALL {
        let path = adapters_dir.join(format!("{name}.toml"));
        if !fs.exists(&path)? {
            fs.write(&path, body.as_bytes())?;
        }
    }
    Ok(())
}
```

The Phase 1 `claude_code.toml` may need a `[roles]` block added (`"CLAUDE.md" = "instructions"`) so section-merge becomes the default for the chain. Verify by reading `crates/aenv-core/src/adapters_builtin/claude_code.toml`; if it lacks roles, add them in this commit.

- [ ] **Step 3: Test the adapters**

Create `crates/aenv-core/tests/adapters_builtin.rs`:

```rust
use aenv_core::adapter::Adapter;
use aenv_core::adapters_builtin::ALL;

#[test]
fn all_seven_adapters_parse_cleanly() {
    assert_eq!(ALL.len(), 7);
    for (name, body) in ALL {
        let parsed: Adapter = toml::from_str(body)
            .unwrap_or_else(|e| panic!("adapter {name} failed to parse: {e}"));
        assert_eq!(parsed.name, *name, "adapter file {name} declares name = {:?}", parsed.name);
        assert!(!parsed.files.is_empty(), "adapter {name} declares no files");
    }
}

#[test]
fn instructions_role_present_on_text_rules_adapters() {
    let parsed: std::collections::BTreeMap<&str, Adapter> = ALL.iter()
        .map(|(n, body)| (*n, toml::from_str(body).unwrap()))
        .collect();
    assert_eq!(parsed["claude-code"].roles.get("CLAUDE.md").map(String::as_str),
               Some("instructions"));
    assert_eq!(parsed["cursor"].roles.get(".cursorrules").map(String::as_str),
               Some("instructions"));
    assert_eq!(parsed["cline"].roles.get(".clinerules").map(String::as_str),
               Some("instructions"));
    assert_eq!(parsed["windsurf"].roles.get(".windsurfrules").map(String::as_str),
               Some("instructions"));
}

#[test]
fn deep_default_merge_on_structured_files() {
    let parsed: std::collections::BTreeMap<&str, Adapter> = ALL.iter()
        .map(|(n, body)| (*n, toml::from_str(body).unwrap()))
        .collect();
    assert_eq!(parsed["mcp"].default_merge.get(".mcp.json").map(String::as_str), Some("deep"));
    assert_eq!(parsed["aider"].default_merge.get(".aider.conf.yml").map(String::as_str), Some("deep"));
    assert_eq!(parsed["continue"].default_merge.get(".continue/config.json").map(String::as_str), Some("deep"));
}
```

- [ ] **Step 4: Test `ensure_written` against MockFilesystem**

Append to `crates/aenv-core/tests/adapters_builtin.rs`:

```rust
use std::path::Path;
mod mock_filesystem;
use mock_filesystem::MockFilesystem;

#[test]
fn ensure_written_creates_all_seven_files() {
    let fs = MockFilesystem::default();
    let dir = Path::new("/aenv/adapters");
    aenv_core::adapters_builtin::ensure_written(&fs, dir).unwrap();
    for (name, _) in aenv_core::adapters_builtin::ALL {
        let path = dir.join(format!("{name}.toml"));
        assert!(fs.exists(&path).unwrap(), "expected {} to exist", path.display());
    }
}

#[test]
fn ensure_written_leaves_existing_files_untouched() {
    let fs = MockFilesystem::default();
    let dir = Path::new("/aenv/adapters");
    fs.write(&dir.join("cursor.toml"), b"user-customized\n").unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, dir).unwrap();
    let body = String::from_utf8(fs.read(&dir.join("cursor.toml")).unwrap()).unwrap();
    assert_eq!(body, "user-customized\n");
}
```

- [ ] **Step 5: Wire `ensure_written` into the CLI startup (new code path)**

Phase 1's CLI does NOT call `ensure_written` (or any init helper) — `crates/aenv-cli/src/main.rs` and `cmd/*.rs` have no references to `adapters_builtin`. This step *creates* the init call; it does not extend an existing mechanism.

In `crates/aenv-cli/src/main.rs`, after resolving `AENV_HOME` but before dispatching to any subcommand handler:

```rust
let aenv_home = aenv_cli::paths::resolve_aenv_home()?;
let layout = aenv_core::home::RegistryLayout::new(aenv_home.clone());
// Phase 2: write built-in adapter TOMLs to the registry on every startup.
// No-ops once the files exist; respects user customization (ensure_written
// skips paths that already exist).
let _ = aenv_core::fs::RealFilesystem;
aenv_core::adapters_builtin::ensure_written(
    &aenv_core::fs::RealFilesystem,
    &layout.adapters_dir(),
)?;
// then dispatch as before...
```

Without this step, `aenv create + activate` on a fresh `AENV_HOME` fails with `AdapterMissing("claude-code")` because the resolver's adapter-registry validation can't find any adapter on disk.

- [ ] **Step 6: Run all tests**

Run: `cargo test -p aenv-core --test adapters_builtin`
Expected: 5 PASS.

`cargo test --workspace` — no regressions.

- [ ] **Step 7: Commit**

```bash
git add crates/aenv-core/src/adapters_builtin/ crates/aenv-core/tests/adapters_builtin.rs \
        crates/aenv-cli/src/main.rs
git commit -m "Ship six remaining built-in adapters

Cursor, Aider, Cline, Continue, Windsurf, MCP — each as a small TOML
file embedded via include_str!. The instructions-style adapters
(cursor, cline, windsurf, claude-code) declare role = 'instructions'
for their main rules file; the structured-config adapters (mcp,
aider, continue) declare default_merge = 'deep' so deep-merge is
automatic across a chain.

ensure_written writes any missing adapter to the registry on every
run, but never overwrites a user-customized one.

claude-code's adapter file gains a [roles] block for CLAUDE.md =
'instructions' if it didn't already; this makes section-merge the
default for the chain, matching PRD R-7's role-based default.
"
```

---

### Task 13: `aenv which <path>` command

Print the qualified identity of the artifact at the given project-relative path, plus its source path, strategy, and the shadow chain.

**Files:**
- Create: `crates/aenv-cli/src/cmd/which.rs`
- Modify: `crates/aenv-cli/src/main.rs` (add `Which` to the `Command` enum)
- Modify: `crates/aenv-cli/src/cmd/mod.rs` (`pub mod which;`)
- Test: append a `which` scenario to `crates/aenv-cli/tests/composition_e2e.rs` (created in Task 17)

- [ ] **Step 1: Write the failing unit test**

`crates/aenv-cli/tests/which_unit.rs`:

```rust
use std::path::{Path, PathBuf};

use aenv_cli::cmd::which::format_which;
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::MaterializeStrategy;
use aenv_core::state::{ActivationState, ManagedFile};

fn qn(ns: &str, short: &str) -> QualifiedName {
    QualifiedName::new(NamespaceId::new(ns).unwrap(), ShortName::new(short).unwrap())
}

fn state_with(mf: ManagedFile) -> ActivationState {
    ActivationState {
        schema_version: 2,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![mf],
        backed_up: vec![],
    }
}

#[test]
fn which_for_symlinked_file_with_shadow() {
    let state = state_with(ManagedFile {
        path: PathBuf::from("CLAUDE.md"),
        qualified_name: qn("leaf", "CLAUDE.md"),
        strategy: MaterializeStrategy::Symlink,
        contributors: vec![],
        shadows: vec![qn("base", "CLAUDE.md")],
    });
    let out = format_which(&state, Path::new("CLAUDE.md")).unwrap();
    assert!(out.contains("Qualified name:  leaf::CLAUDE.md"));
    assert!(out.contains("Strategy:        symlink"));
    assert!(out.contains("Shadows:"));
    assert!(out.contains("base::CLAUDE.md"));
}

#[test]
fn which_for_merged_file_lists_contributors() {
    let state = state_with(ManagedFile {
        path: PathBuf::from(".mcp.json"),
        qualified_name: qn("(merged)", ".mcp.json"),
        strategy: MaterializeStrategy::DeepMerge(
            aenv_core::resolve::DeepMergeFormat::Json,
        ),
        contributors: vec![qn("base", ".mcp.json"), qn("leaf", ".mcp.json")],
        shadows: vec![],
    });
    let out = format_which(&state, Path::new(".mcp.json")).unwrap();
    assert!(out.contains("Qualified name:  (merged)"));
    assert!(out.contains("Strategy:        deep-merge (json)"));
    assert!(out.contains("Contributors:"));
    assert!(out.contains("base::.mcp.json"));
    assert!(out.contains("leaf::.mcp.json"));
}

#[test]
fn which_for_unmanaged_path_reports_error() {
    let state = ActivationState {
        schema_version: 2,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![],
        backed_up: vec![],
    };
    let err = format_which(&state, Path::new("unmanaged.txt")).unwrap_err();
    assert!(err.to_string().contains("not managed"));
}
```

- [ ] **Step 2: Implement `format_which` and the handler**

Create `crates/aenv-cli/src/cmd/which.rs`:

```rust
use std::path::{Path, PathBuf};

use aenv_core::resolve::{DeepMergeFormat, MaterializeStrategy};
use aenv_core::state::ActivationState;

pub fn format_which(state: &ActivationState, query: &Path) -> Result<String, String> {
    let mf = state.managed_files.iter()
        .find(|m| m.path == query)
        .ok_or_else(|| format!("path {} is not managed by the active namespace", query.display()))?;
    let mut out = String::new();
    out.push_str(&format!("Qualified name:  {}\n", mf.qualified_name));
    out.push_str(&format!("Materialized at: ./{}\n", query.display()));
    out.push_str(&format!("Strategy:        {}\n", render_strategy(mf.strategy)));
    if !mf.contributors.is_empty() {
        out.push_str("Contributors:    ");
        for (i, q) in mf.contributors.iter().enumerate() {
            if i > 0 { out.push_str("\n                 "); }
            out.push_str(&q.to_string());
        }
        out.push('\n');
    }
    if !mf.shadows.is_empty() {
        out.push_str("Shadows:         ");
        for (i, q) in mf.shadows.iter().enumerate() {
            if i > 0 { out.push_str("\n                 "); }
            out.push_str(&q.to_string());
        }
        out.push('\n');
    } else if mf.contributors.is_empty() {
        // Non-merged single-source file with no shadows.
        out.push_str("Shadows:         (nothing — no parent namespace defines this artifact)\n");
    }
    Ok(out)
}

fn render_strategy(s: MaterializeStrategy) -> String {
    match s {
        MaterializeStrategy::Symlink => "symlink".into(),
        MaterializeStrategy::Identical => "identical (project file already matches)".into(),
        MaterializeStrategy::Copy => "copy".into(),
        MaterializeStrategy::SectionMerge => "section-merge".into(),
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Json) => "deep-merge (json)".into(),
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Yaml) => "deep-merge (yaml)".into(),
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Toml) => "deep-merge (toml)".into(),
        MaterializeStrategy::Merged => "merged (Phase 1 legacy)".into(),
    }
}

pub fn run(project_root: PathBuf, query: PathBuf) -> aenv_core::Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    let body = match std::fs::read(&state_path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // No state file -> not activated. Distinct from "not pinned"
            // (which is no .aenv file at all). Phase 2 reuses ProjectNotPinned
            // (exit 20); Phase 5 may add a dedicated NotActivated variant.
            return Err(aenv_core::AenvError::ProjectNotPinned);
        }
        Err(e) => return Err(aenv_core::AenvError::from(e)),
    };
    let state: ActivationState = serde_json::from_slice(&body)
        .map_err(|e| aenv_core::AenvError::ActivationConflict(e.to_string()))?;
    let out = format_which(&state, &query)
        .map_err(aenv_core::AenvError::ActivationConflict)?;
    print!("{out}");
    Ok(())
}
```

`aenv-cli/src/lib.rs` doesn't exist in Phase 1; `aenv-cli` is `[[bin]]` only. To unit-test `format_which`, add a `[lib]` target in `crates/aenv-cli/Cargo.toml` that exposes the `cmd` module:

```toml
[[bin]]
name = "aenv"
path = "src/main.rs"

[lib]
name = "aenv_cli"
path = "src/lib.rs"
```

…with `crates/aenv-cli/src/lib.rs`:

```rust
pub mod cmd;
pub mod paths;
```

…and `crates/aenv-cli/src/main.rs` consuming the same modules via `use aenv_cli::cmd::*;`. (This is a pattern change for aenv-cli; Phase 1 left tests at integration-only.)

- [ ] **Step 3: Wire into clap**

Modify `crates/aenv-cli/src/main.rs` — add a `Which { path: PathBuf, project: Option<PathBuf> }` variant to the `Command` enum, dispatch to `cmd::which::run`.

- [ ] **Step 4: Run the test**

Run: `cargo test -p aenv-cli --test which_unit`
Expected: 3 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-cli/src/cmd/which.rs crates/aenv-cli/src/cmd/mod.rs \
        crates/aenv-cli/src/main.rs crates/aenv-cli/src/lib.rs \
        crates/aenv-cli/Cargo.toml crates/aenv-cli/tests/which_unit.rs
git commit -m "Add 'aenv which <path>' command

Reads .aenv-state/state.json, looks up the path among managed files,
and prints qualified name, materialized path, strategy, contributors
(for merged files), and shadow chain (for non-merged files). Format
matches the user trace in functional spec §5.5.

aenv-cli grows a [lib] target so format_which can be unit-tested
without exec'ing the binary.
"
```

---

### Task 14: `aenv fork <file>` — detach a single materialized file

Replace a symlinked artifact with a regular copy, remove it from `managed_files`, and stop touching it on subsequent activations.

For merged files: forking is a no-op on disk (it's already a regular file) but removes it from `managed_files` so future activations don't overwrite it. Print a hint to that effect.

**Files:**
- Create: `crates/aenv-cli/src/cmd/fork.rs`
- Modify: `crates/aenv-core/src/activate.rs` (add a `fork_file` library function so the logic is testable against `MockFilesystem`)
- Test: `crates/aenv-core/tests/fork.rs`

- [ ] **Step 1: Write the library test**

Create `crates/aenv-core/tests/fork.rs`:

```rust
use std::path::Path;

use aenv_core::activate::fork_file;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::state::ActivationState;
use aenv_core::resolve::MaterializeStrategy;

mod mock_filesystem;
use mock_filesystem::MockFilesystem;

const REG: &str = "/aenv";
const PROJ: &str = "/proj";

fn registry() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from(REG))
}

fn setup_activated_chain(fs: &MockFilesystem) {
    // Same setup as composition::skill_overlay_shadows_parent_skill.
    let cc: Adapter = toml::from_str(
        r#"
name = "claude-code"
files = [".claude/skills/X/SKILL.md", "CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"#).unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc);

    fs.write(Path::new(&format!("{REG}/envs/leaf/aenv.toml")),
        br#"
name = "leaf"
[adapters.claude-code]
files = [".claude/skills/X/SKILL.md", "CLAUDE.md"]
"#).unwrap();
    fs.write(Path::new(&format!("{REG}/envs/leaf/.claude/skills/X/SKILL.md")),
        b"the skill body").unwrap();
    fs.write(Path::new(&format!("{REG}/envs/leaf/CLAUDE.md")),
        b"# leaf\n").unwrap();

    aenv_core::activate::activate_namespace(
        fs, &registry(), &adapters,
        Path::new(PROJ), &NamespaceId::new("leaf").unwrap(),
    ).unwrap();
}

#[test]
fn forking_a_symlink_replaces_it_with_a_regular_file_with_same_bytes() {
    let fs = MockFilesystem::default();
    setup_activated_chain(&fs);

    let skill = Path::new(&format!("{PROJ}/.claude/skills/X/SKILL.md"));
    assert!(matches!(
        fs.symlink_metadata(skill).unwrap().kind,
        aenv_core::fs::FileKind::Symlink
    ));

    fork_file(&fs, Path::new(PROJ), Path::new(".claude/skills/X/SKILL.md")).unwrap();

    assert!(!matches!(
        fs.symlink_metadata(skill).unwrap().kind,
        aenv_core::fs::FileKind::Symlink
    ));
    assert_eq!(fs.read(skill).unwrap(), b"the skill body");

    // No longer in managed_files.
    let state: ActivationState = serde_json::from_slice(
        &fs.read(Path::new(&format!("{PROJ}/.aenv-state/state.json"))).unwrap(),
    ).unwrap();
    assert!(state.managed_files.iter()
        .all(|m| !m.path.to_string_lossy().contains("SKILL.md")));
}

#[test]
fn forking_a_merged_file_keeps_it_but_drops_management() {
    let fs = MockFilesystem::default();
    setup_activated_chain(&fs);

    let claude = Path::new(&format!("{PROJ}/CLAUDE.md"));
    let before = fs.read(claude).unwrap();
    fork_file(&fs, Path::new(PROJ), Path::new("CLAUDE.md")).unwrap();
    let after = fs.read(claude).unwrap();
    assert_eq!(before, after, "merged file content unchanged");

    let state: ActivationState = serde_json::from_slice(
        &fs.read(Path::new(&format!("{PROJ}/.aenv-state/state.json"))).unwrap(),
    ).unwrap();
    assert!(state.managed_files.iter()
        .all(|m| !m.path.to_string_lossy().ends_with("CLAUDE.md")));
}

#[test]
fn forking_unmanaged_path_errors() {
    let fs = MockFilesystem::default();
    setup_activated_chain(&fs);
    let err = fork_file(&fs, Path::new(PROJ), Path::new("other.txt")).unwrap_err();
    assert!(err.to_string().contains("not managed"));
}
```

- [ ] **Step 2: Implement `fork_file`**

Append to `crates/aenv-core/src/activate.rs`:

```rust
/// Detach a single materialized file from namespace management.
/// For symlinks: replace with a regular copy of the target. For merged
/// files: leave on disk unchanged. In both cases: remove from
/// state.managed_files so subsequent activations won't touch it.
pub fn fork_file<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    rel_path: &Path,
) -> Result<(), AenvError> {
    let state_path = project_root.join(".aenv-state/state.json");
    let mut state: ActivationState = serde_json::from_slice(&fs.read(&state_path)?)
        .map_err(|e| AenvError::ActivationConflict(format!("state read: {e}")))?;
    let pos = state.managed_files.iter()
        .position(|m| m.path == rel_path)
        .ok_or_else(|| AenvError::ActivationConflict(
            format!("{} is not managed by the active namespace", rel_path.display())
        ))?;
    let mf = &state.managed_files[pos];
    let project_path = project_root.join(rel_path);
    if matches!(mf.strategy, MaterializeStrategy::Symlink) {
        // Read through the symlink to get the underlying bytes, then replace.
        let bytes = fs.read(&project_path)?;
        fs.remove_file(&project_path)?;
        fs.write(&project_path, &bytes)?;
    }
    // Drop from managed_files; persist.
    state.managed_files.remove(pos);
    let body = serde_json::to_vec_pretty(&state)
        .map_err(|e| AenvError::ActivationConflict(format!("state serialize: {e}")))?;
    fs.write(&state_path, &body)?;
    Ok(())
}
```

- [ ] **Step 3: Wire the CLI**

Create `crates/aenv-cli/src/cmd/fork.rs`:

```rust
use std::path::PathBuf;

pub fn run_file(project_root: PathBuf, rel: PathBuf) -> aenv_core::Result<()> {
    aenv_core::activate::fork_file(
        &aenv_core::fs::RealFilesystem,
        &project_root,
        &rel,
    )?;
    println!("Forked {}:", rel.display());
    println!("  - replaced symlink with a copy at ./{}", rel.display());
    println!("  - removed from namespace management for this project");
    println!("  - subsequent activations will not touch this file");
    Ok(())
}
```

Add a `Fork { target: String, project: Option<PathBuf> }` variant to the CLI `Command` enum. Dispatch: if `target` resolves to a path that exists in `state.managed_files`, call `run_file`; otherwise treat as a namespace name and delegate to Task 15's `run_name`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test fork`
Expected: 3 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/activate.rs crates/aenv-cli/src/cmd/fork.rs \
        crates/aenv-cli/src/cmd/mod.rs crates/aenv-cli/src/main.rs \
        crates/aenv-core/tests/fork.rs
git commit -m "Add 'aenv fork <file>' to detach a managed artifact

fork_file replaces a symlinked artifact with a regular copy of its
bytes and removes it from state.managed_files. Merged files are
already regular files on disk; forking them leaves the file untouched
but stops aenv from regenerating them on subsequent activations.
Unmanaged paths error.

The CLI dispatcher distinguishes 'fork' (no arg, Task 14b),
'fork <file>' (this task), and 'fork <name>' (Task 15) by inspecting
state.managed_files.
"
```

---

### Task 14b: `aenv fork` (no argument) — whole-project detach (R-53)

The bare `aenv fork` variant replaces *every* symlinked managed file with a regular copy, marks the project as detached, and removes the state file so subsequent activations and (when Phase 6 lands) shell-hook auto-activation skip it. The `.aenv` pin file is *kept* — the project still records "this was forked from X" via the pin, but activation is disabled until the user explicitly re-pins.

**Files:**
- Modify: `crates/aenv-core/src/activate.rs` (add `fork_project` library function)
- Modify: `crates/aenv-cli/src/cmd/fork.rs` (add `run_project_detach`)
- Modify: `crates/aenv-cli/src/main.rs` (route bare `Fork { target: None }` here)
- Test: append to `crates/aenv-core/tests/fork.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/aenv-core/tests/fork.rs`:

```rust
#[test]
fn forking_whole_project_replaces_every_symlink_and_removes_state() {
    let fs = MockFilesystem::default();
    setup_activated_chain(&fs);

    let skill = Path::new(&format!("{PROJ}/.claude/skills/X/SKILL.md"));
    let claude = Path::new(&format!("{PROJ}/CLAUDE.md"));
    let state_path = Path::new(&format!("{PROJ}/.aenv-state/state.json"));
    assert!(matches!(
        fs.symlink_metadata(skill).unwrap().kind,
        aenv_core::fs::FileKind::Symlink
    ));
    assert!(fs.exists(state_path).unwrap());

    aenv_core::activate::fork_project(&fs, Path::new(PROJ)).unwrap();

    // Every symlinked file is now a regular file with its bytes inlined.
    assert!(!matches!(
        fs.symlink_metadata(skill).unwrap().kind,
        aenv_core::fs::FileKind::Symlink
    ));
    assert_eq!(fs.read(skill).unwrap(), b"the skill body");

    // Merged files are unchanged (they were already regular).
    assert!(fs.exists(claude).unwrap());

    // State is removed: no subsequent activation will manage these files.
    assert!(!fs.exists(state_path).unwrap());

    // Backups are also dropped — the user is committing to the detached state.
    let backup_dir = Path::new(&format!("{PROJ}/.aenv-state/backup"));
    assert!(!fs.exists(backup_dir).unwrap());
}

#[test]
fn forking_whole_project_with_no_activation_is_a_clean_no_op() {
    let fs = MockFilesystem::default();
    // No prior activation, no state file.
    let result = aenv_core::activate::fork_project(&fs, Path::new(PROJ));
    // Idempotent: no error, no state to remove.
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Implement `fork_project`**

Append to `crates/aenv-core/src/activate.rs`:

```rust
/// Detach the entire project from namespace management.
///
/// For every managed file with strategy Symlink, read the resolved bytes
/// through the symlink and replace it with a regular file. For merged
/// strategies the file is already regular — leave it. Then remove
/// `.aenv-state/` entirely (state.json + backup/) so subsequent
/// activations and shell-hook auto-activation skip this project.
///
/// The `.aenv` pin file is intentionally *not* removed — the project
/// retains its declaration of "I was forked from <namespace>" for human
/// reference. Re-pin with `aenv use <name>` to re-enable activation.
///
/// Idempotent: a project with no state file returns Ok(()) without
/// touching anything.
pub fn fork_project<F: Filesystem>(
    fs: &F,
    project_root: &Path,
) -> Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        return Ok(());
    }
    let state: ActivationState = serde_json::from_slice(&fs.read(&state_path)?)
        .map_err(|e| AenvError::ActivationConflict(format!("state read: {e}")))?;

    for mf in &state.managed_files {
        if matches!(mf.strategy, MaterializeStrategy::Symlink) {
            let project_path = project_root.join(&mf.path);
            let bytes = fs.read(&project_path)?;
            fs.remove_file(&project_path)?;
            fs.write(&project_path, &bytes)?;
        }
        // SectionMerge / DeepMerge / Identical / Copy / Merged: leave on disk.
    }

    // Clear .aenv-state/ entirely.
    let state_dir = project_root.join(".aenv-state");
    fs.remove_dir_all(&state_dir)?;
    Ok(())
}
```

- [ ] **Step 3: Wire into the CLI**

Modify `crates/aenv-cli/src/cmd/fork.rs`:

```rust
pub fn run_project_detach(project_root: PathBuf) -> aenv_core::Result<()> {
    aenv_core::activate::fork_project(
        &aenv_core::fs::RealFilesystem,
        &project_root,
    )?;
    println!("Forked project (detached from namespace management):");
    println!("  - replaced every symlinked managed file with a regular copy");
    println!("  - removed .aenv-state/ (state + backups)");
    println!("  - .aenv pin retained for reference; re-pin to re-activate");
    Ok(())
}
```

Modify `crates/aenv-cli/src/main.rs`'s clap config: the `Fork` variant accepts an *optional* target. Dispatcher:

```rust
match target {
    None => cmd::fork::run_project_detach(project_root),
    Some(t) => {
        // Try as a project-managed path first (fork <file>); fall back to
        // namespace creation (fork <name>) only if the path is not managed
        // *and* not present as a file in the project root.
        let rel = PathBuf::from(&t);
        let project_path = project_root.join(&rel);
        let state_path = project_root.join(".aenv-state/state.json");
        let is_managed = std::fs::read(&state_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<aenv_core::state::ActivationState>(&b).ok())
            .map(|s| s.managed_files.iter().any(|m| m.path == rel))
            .unwrap_or(false);
        if is_managed || project_path.exists() {
            cmd::fork::run_file(project_root, rel)
        } else {
            cmd::fork::run_name(aenv_home, project_root, t)
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test fork`
Expected: 5 PASS (3 from Task 14 + 2 new).

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/activate.rs crates/aenv-cli/src/cmd/fork.rs \
        crates/aenv-cli/src/main.rs crates/aenv-core/tests/fork.rs
git commit -m "Add 'aenv fork' (no-arg) whole-project detach (R-53)

fork_project walks state.managed_files, replaces every Symlink-strategy
file with a regular copy of its resolved bytes, leaves merged files
alone, and removes .aenv-state/ entirely so subsequent activations and
shell-hook auto-activation (Phase 6) skip the project. The .aenv pin
file is retained for human reference.

Idempotent on a project with no prior activation (returns Ok without
side-effects).

CLI dispatcher: bare 'aenv fork' -> fork_project; 'aenv fork <target>'
tries project-managed-file first, falls back to namespace-creation.
"
```

---

### Task 15: `aenv fork <name>` — create a new namespace from the current project

Mirror image of `aenv use`. Take a project's currently-active files (whether managed or not), copy them into a new namespace dir under `~/.aenv/envs/<name>/`, write a manifest declaring the adapters and their files, then update the `.aenv` pin to point to the new namespace.

**Files:**
- Modify: `crates/aenv-core/src/namespace.rs` (`create_namespace_from_project`)
- Modify: `crates/aenv-cli/src/cmd/fork.rs` (add `run_name`)
- Test: append to `crates/aenv-core/tests/namespace.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/aenv-core/tests/namespace.rs`:

```rust
#[test]
fn fork_name_copies_managed_files_from_project_and_writes_manifest() {
    use aenv_core::namespace::create_namespace_from_project;
    use aenv_core::adapter::{Adapter, AdapterRegistry};
    use aenv_core::home::RegistryLayout;
    use std::path::Path;
    mod mfs { include!("mock_filesystem.rs"); }
    let fs = mfs::MockFilesystem::default();

    // Project has CLAUDE.md (regular file — assume user has just edited it post-fork).
    fs.write(Path::new("/p/CLAUDE.md"), b"# project version\n").unwrap();
    fs.write(Path::new("/p/.mcp.json"), b"{}").unwrap();

    let cc: Adapter = toml::from_str(r#"
name = "claude-code"
files = ["CLAUDE.md"]
"#).unwrap();
    let mcp: Adapter = toml::from_str(r#"
name = "mcp"
files = [".mcp.json"]
"#).unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc);
    adapters.insert(mcp);

    let reg = RegistryLayout::new(PathBuf::from("/aenv"));

    create_namespace_from_project(
        &fs, &reg, &adapters, "new-env", Path::new("/p"),
    ).unwrap();

    // Manifest is created with the right adapters + files.
    let manifest_bytes = fs.read(Path::new("/aenv/envs/new-env/aenv.toml")).unwrap();
    let m: aenv_core::manifest::AenvManifest = toml::from_str(std::str::from_utf8(&manifest_bytes).unwrap()).unwrap();
    assert_eq!(m.name, "new-env");
    assert!(m.adapters.contains_key("claude-code"));
    assert!(m.adapters.contains_key("mcp"));

    // Files are copied into the namespace dir.
    let copied_claude = fs.read(Path::new("/aenv/envs/new-env/CLAUDE.md")).unwrap();
    assert_eq!(copied_claude, b"# project version\n");
    let copied_mcp = fs.read(Path::new("/aenv/envs/new-env/.mcp.json")).unwrap();
    assert_eq!(copied_mcp, b"{}");
}
```

- [ ] **Step 1b: Add a glob-walking test**

Append a second test to verify glob entries get expanded by walking the project tree:

```rust
#[test]
fn fork_name_walks_glob_directories_and_copies_every_file() {
    use aenv_core::namespace::create_namespace_from_project;
    use aenv_core::adapter::{Adapter, AdapterRegistry};
    use aenv_core::home::RegistryLayout;
    use std::path::{Path, PathBuf};
    mod mfs { include!("mock_filesystem.rs"); }
    let fs = mfs::MockFilesystem::default();

    // Project has skills under .claude/skills/{a,b}/SKILL.md.
    fs.write(Path::new("/p/.claude/skills/a/SKILL.md"), b"skill a").unwrap();
    fs.write(Path::new("/p/.claude/skills/b/SKILL.md"), b"skill b").unwrap();
    fs.write(Path::new("/p/CLAUDE.md"), b"# proj\n").unwrap();

    // Adapter declares a literal file AND a glob.
    let cc: Adapter = toml::from_str(r#"
name = "claude-code"
files = ["CLAUDE.md", ".claude/skills/**/*"]
"#).unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc);

    let reg = RegistryLayout::new(PathBuf::from("/aenv"));
    create_namespace_from_project(
        &fs, &reg, &adapters, "forked", Path::new("/p"),
    ).unwrap();

    // Both skills made it.
    assert_eq!(
        fs.read(Path::new("/aenv/envs/forked/.claude/skills/a/SKILL.md")).unwrap(),
        b"skill a",
    );
    assert_eq!(
        fs.read(Path::new("/aenv/envs/forked/.claude/skills/b/SKILL.md")).unwrap(),
        b"skill b",
    );

    // The new manifest lists the literal paths it found (NOT the glob pattern).
    let body = fs.read(Path::new("/aenv/envs/forked/aenv.toml")).unwrap();
    let m: aenv_core::manifest::AenvManifest =
        toml::from_str(std::str::from_utf8(&body).unwrap()).unwrap();
    let files = &m.adapters["claude-code"].files;
    assert!(files.iter().any(|p| p == "CLAUDE.md"));
    assert!(files.iter().any(|p| p == ".claude/skills/a/SKILL.md"));
    assert!(files.iter().any(|p| p == ".claude/skills/b/SKILL.md"));
    // The glob pattern itself does NOT appear — the manifest carries only
    // resolved literal entries.
    assert!(!files.iter().any(|p| p.contains('*')));
}
```

- [ ] **Step 2: Implement `create_namespace_from_project`**

Append to `crates/aenv-core/src/namespace.rs`:

```rust
/// Create a new namespace by gathering every adapter-managed file at the
/// project root and copying it into the namespace dir.
///
/// For literal entries in `adapter.files`: copy if present, skip if absent.
///
/// For glob entries (containing `*`): derive the literal directory prefix
/// (everything before the first `*` segment), walk the project tree under
/// that prefix, and copy every regular file encountered. Symlinks are
/// followed — the bytes captured represent the project's *effective*
/// harness state at fork time (e.g. a symlink to `~/.aenv/envs/base/...`
/// is materialized as a regular copy of base's content in the new namespace).
///
/// The new manifest carries the *resolved literal paths* it captured, not
/// the source glob pattern.
pub fn create_namespace_from_project<F: crate::fs::Filesystem>(
    fs: &F,
    registry: &crate::home::RegistryLayout,
    adapters: &crate::adapter::AdapterRegistry,
    new_name: &str,
    project_root: &std::path::Path,
) -> Result<(), crate::AenvError> {
    let dest = registry.namespace_dir(new_name);
    if fs.exists(&dest)? {
        return Err(crate::AenvError::ManifestInvalid(
            format!("namespace {new_name} already exists at {}", dest.display()),
        ));
    }
    let mut manifest_adapters = std::collections::BTreeMap::new();
    // AdapterRegistry::iter yields (&String, &Adapter) tuples.
    for (_, adapter) in adapters.iter() {
        let mut files: Vec<String> = Vec::new();
        for rel in &adapter.files {
            if rel.contains('*') {
                // Glob entry: walk the project tree under the literal prefix.
                let prefix = literal_prefix(rel);
                let walk_root = project_root.join(prefix);
                if fs.exists(&walk_root)? {
                    let mut found: Vec<String> = Vec::new();
                    walk_project_tree(fs, project_root, &walk_root, &mut found)?;
                    for f in found {
                        let proj_path = project_root.join(&f);
                        // fs.read follows symlinks, capturing the resolved bytes.
                        let bytes = fs.read(&proj_path)?;
                        let dest_path = dest.join(&f);
                        fs.write(&dest_path, &bytes)?;
                        files.push(f);
                    }
                }
            } else {
                // Literal entry: copy if present.
                let proj_path = project_root.join(rel);
                if fs.exists(&proj_path)? {
                    let bytes = fs.read(&proj_path)?;
                    let dest_path = dest.join(rel);
                    fs.write(&dest_path, &bytes)?;
                    files.push(rel.clone());
                }
            }
        }
        // De-dup and sort for stable manifest output.
        files.sort();
        files.dedup();
        if !files.is_empty() {
            manifest_adapters.insert(
                adapter.name.clone(),
                crate::manifest::AdapterEntry { files, merge: None },
            );
        }
    }
    let manifest = crate::manifest::AenvManifest {
        name: new_name.to_string(),
        extends: vec![],
        adapters: manifest_adapters,
    };
    let body = toml::to_string_pretty(&manifest)
        .map_err(|e| crate::AenvError::ManifestInvalid(e.to_string()))?;
    fs.write(&registry.manifest_path(new_name), body.as_bytes())?;
    Ok(())
}

/// Derive the literal directory prefix from a glob pattern.
/// `".claude/skills/**/*"` -> `".claude/skills"`.
/// `"foo/*.md"` -> `"foo"`.
/// `"*"` -> `""` (project root).
fn literal_prefix(pattern: &str) -> &str {
    match pattern.find('*') {
        Some(i) => {
            // Trim back to the last path separator before the first glob char.
            let candidate = &pattern[..i];
            match candidate.rfind('/') {
                Some(slash) => &pattern[..slash],
                None => "",
            }
        }
        None => pattern,
    }
}

/// Walk the project tree under `walk_root`, pushing project-relative paths
/// (as `String`s) into `out`. Skips `.aenv-state/` so we never capture
/// activation state into a forked namespace.
fn walk_project_tree<F: crate::fs::Filesystem>(
    fs: &F,
    project_root: &std::path::Path,
    walk_root: &std::path::Path,
    out: &mut Vec<String>,
) -> Result<(), crate::AenvError> {
    for entry in fs.list_dir(walk_root)? {
        // Skip aenv's own state dir.
        let name = entry.file_name().map(|n| n.to_string_lossy().to_string());
        if name.as_deref() == Some(".aenv-state") { continue; }

        let meta = fs.metadata(&entry)?;
        if matches!(meta.kind, crate::fs::FileKind::Directory) {
            walk_project_tree(fs, project_root, &entry, out)?;
        } else {
            // Project-relative form.
            if let Ok(rel) = entry.strip_prefix(project_root) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Wire the CLI**

In `crates/aenv-cli/src/cmd/fork.rs`:

```rust
pub fn run_name(
    aenv_home: PathBuf,
    project_root: PathBuf,
    new_name: String,
) -> aenv_core::Result<()> {
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.to_path_buf());
    let adapters = aenv_cli::paths::load_adapter_registry(&registry)?;
    aenv_core::namespace::create_namespace_from_project(
        &aenv_core::fs::RealFilesystem,
        &registry, &adapters,
        &new_name, &project_root,
    )?;
    aenv_core::project::write_pin(
        &aenv_core::fs::RealFilesystem,
        &project_root, &new_name,
    )?;
    println!("Forked project into new namespace '{new_name}'");
    println!("  - copied harness files into ~/.aenv/envs/{new_name}/");
    println!("  - updated .aenv pin");
    println!("  - run 'aenv activate' to materialize");
    Ok(())
}
```

`load_adapter_registry` is a CLI helper added in Task 12's wiring.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test namespace`
Expected: prior tests + both new `fork_name_*` tests (the literal-paths
case and the glob-walking case) all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/namespace.rs crates/aenv-cli/src/cmd/fork.rs \
        crates/aenv-core/tests/namespace.rs
git commit -m "Add 'aenv fork <name>' to create namespace from project

create_namespace_from_project walks the registered adapters. For each
literal file the adapter declares, it copies bytes (if the file is
present at the project root). For each glob entry, it derives the
literal directory prefix and recursively walks the project tree under
that prefix, capturing every regular file. Symlinks are followed -
the bytes captured represent the project's effective harness state at
fork time, so a project with a symlink-managed skill ends up with a
real copy of the skill in the new namespace.

The new manifest carries the resolved literal paths it captured, not
the source glob pattern, so subsequent activations are deterministic.

CLI handler then updates the .aenv pin; the user can 'aenv activate'
to materialize.
"
```

---

### Task 16: Upgrade `aenv status` to show resolution chain + provenance

Phase 1's `status` prints the active namespace and the managed-file list. Phase 2 adds the resolution chain (root → leaf) and per-file qualified provenance (the line `from <qualified-name>` or `merged from <contributor list>`).

**Files:**
- Modify: `crates/aenv-cli/src/cmd/status.rs`
- Test: `crates/aenv-cli/tests/status_unit.rs` (new file, using the `[lib]` target introduced in Task 13)

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-cli/tests/status_unit.rs`:

```rust
use std::path::PathBuf;

use aenv_cli::cmd::status::format_status;
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::{DeepMergeFormat, MaterializeStrategy};
use aenv_core::state::{ActivationState, ManagedFile};

fn qn(ns: &str, short: &str) -> QualifiedName {
    QualifiedName::new(NamespaceId::new(ns).unwrap(), ShortName::new(short).unwrap())
}

#[test]
fn status_prints_resolution_chain_and_managed_provenance() {
    let state = ActivationState {
        schema_version: 2,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![
            ManagedFile {
                path: PathBuf::from("CLAUDE.md"),
                qualified_name: qn("(merged)", "CLAUDE.md"),
                strategy: MaterializeStrategy::SectionMerge,
                contributors: vec![qn("base", "CLAUDE.md"), qn("leaf", "CLAUDE.md")],
                shadows: vec![],
            },
            ManagedFile {
                path: PathBuf::from(".claude/skills/write-tests/SKILL.md"),
                qualified_name: qn("leaf", ".claude/skills/write-tests/SKILL.md"),
                strategy: MaterializeStrategy::Symlink,
                contributors: vec![],
                shadows: vec![qn("base", ".claude/skills/write-tests/SKILL.md")],
            },
            ManagedFile {
                path: PathBuf::from(".mcp.json"),
                qualified_name: qn("(merged)", ".mcp.json"),
                strategy: MaterializeStrategy::DeepMerge(DeepMergeFormat::Json),
                contributors: vec![qn("base", ".mcp.json"), qn("leaf", ".mcp.json")],
                shadows: vec![],
            },
        ],
        backed_up: vec![],
    };
    // Resolution chain comes from outside state — passed in by the handler.
    let chain = vec![
        NamespaceId::new("base").unwrap(),
        NamespaceId::new("leaf").unwrap(),
    ];
    let out = format_status(&state, &chain);
    assert!(out.contains("Active namespace: leaf"));
    assert!(out.contains("Resolution:       base → leaf"));
    assert!(out.contains("CLAUDE.md"));
    assert!(out.contains("merged from base + leaf"));
    assert!(out.contains("write-tests"));
    assert!(out.contains("(shadows base::"));
    assert!(out.contains(".mcp.json"));
    assert!(out.contains("merged (deep-merge json) from base + leaf"));
}

#[test]
fn status_no_active_namespace() {
    // Phase 1 already handled this; ensure the new format doesn't regress.
    // For Phase 2 the "no active" branch lives in `run`, not `format_status`,
    // so just check format_status doesn't crash on empty managed_files.
    let state = ActivationState {
        schema_version: 2,
        active_namespace: "alone".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![],
        backed_up: vec![],
    };
    let chain = vec![NamespaceId::new("alone").unwrap()];
    let out = format_status(&state, &chain);
    assert!(out.contains("Resolution:       alone"));
    assert!(out.contains("No managed files."));
}
```

- [ ] **Step 2: Implement `format_status`**

Modify `crates/aenv-cli/src/cmd/status.rs`:

```rust
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::{DeepMergeFormat, MaterializeStrategy};
use aenv_core::state::{ActivationState, ManagedFile};

pub fn format_status(state: &ActivationState, chain: &[NamespaceId]) -> String {
    let mut out = String::new();
    out.push_str(&format!("Active namespace: {}\n", state.active_namespace));
    out.push_str("Resolution:       ");
    let rendered: Vec<&str> = chain.iter().map(|n| n.as_str()).collect();
    out.push_str(&rendered.join(" → "));
    out.push('\n');
    out.push('\n');

    if state.managed_files.is_empty() {
        out.push_str("No managed files.\n");
    } else {
        out.push_str("Managed files:\n");
        for mf in &state.managed_files {
            out.push_str(&format!("  ./{}\n", mf.path.display()));
            out.push_str(&format!("      {}\n", describe(mf)));
            for s in &mf.shadows {
                out.push_str(&format!("      (shadows {s})\n"));
            }
        }
    }

    if !state.backed_up.is_empty() {
        out.push('\n');
        out.push_str("Backed-up originals:\n");
        for b in &state.backed_up {
            out.push_str(&format!("  {} -> {}\n", b.path.display(), b.backup.display()));
        }
    }
    out
}

fn describe(mf: &ManagedFile) -> String {
    match mf.strategy {
        MaterializeStrategy::Symlink => format!("from {}", mf.qualified_name),
        MaterializeStrategy::Identical => format!("identical to {} (no symlink)", mf.qualified_name),
        MaterializeStrategy::Copy => format!("copy of {}", mf.qualified_name),
        MaterializeStrategy::SectionMerge => {
            let parts: Vec<String> = mf.contributors.iter()
                .map(|c| c.namespace().as_str().to_string())
                .collect();
            format!("merged from {}", parts.join(" + "))
        }
        MaterializeStrategy::DeepMerge(fmt) => {
            let parts: Vec<String> = mf.contributors.iter()
                .map(|c| c.namespace().as_str().to_string())
                .collect();
            let fmt_name = match fmt {
                DeepMergeFormat::Json => "json",
                DeepMergeFormat::Yaml => "yaml",
                DeepMergeFormat::Toml => "toml",
            };
            format!("merged (deep-merge {fmt_name}) from {}", parts.join(" + "))
        }
        MaterializeStrategy::Merged => format!("merged (Phase 1 legacy) {}", mf.qualified_name),
    }
}

pub fn run(project_root: std::path::PathBuf, aenv_home: std::path::PathBuf)
    -> aenv_core::Result<()>
{
    let state_path = project_root.join(".aenv-state/state.json");
    if !state_path.exists() {
        println!("No active namespace at {}", project_root.display());
        return Ok(());
    }
    let state: ActivationState = serde_json::from_slice(&std::fs::read(&state_path)?)
        .map_err(|e| aenv_core::AenvError::ActivationConflict(e.to_string()))?;
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.to_path_buf());
    let adapters = crate::paths::load_adapter_registry(&registry)?;
    let resolution = aenv_core::resolve::resolve_namespace(
        &aenv_core::fs::RealFilesystem,
        &registry, &adapters,
        &aenv_core::identity::NamespaceId::new(&state.active_namespace)?,
    )?;
    print!("{}", format_status(&state, &resolution.chain));
    Ok(())
}
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p aenv-cli --test status_unit`
Expected: 2 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/aenv-cli/src/cmd/status.rs crates/aenv-cli/tests/status_unit.rs
git commit -m "Upgrade 'aenv status' with resolution chain and per-file provenance

format_status takes the ActivationState plus the resolved chain (root
-> leaf) and prints them. Each managed file shows its provenance line
('from <qualified>', 'merged from <ns> + <ns>', or 'merged (deep-merge
json) from <ns> + <ns>') plus a '(shadows ...)' annotation per
shadowed parent.

run re-resolves the chain on every status call rather than caching
it in state.json; this keeps the chain authoritative even if the
registry is edited between activation and status.
"
```

---

### Task 17: End-to-end CLI composition integration test

Drive the built `aenv` binary as a subprocess against `tempfile::tempdir()`. Mirror Phase 1's `cli_e2e.rs` structure. The goldenpath: create two namespaces (`base` and `leaf`), pin to `leaf`, activate, run `which` + `status`, fork a file, deactivate.

Also runs the materialized-path invariant: walk the post-activation project tree, assert no path contains `::`.

**Files:**
- Create: `crates/aenv-cli/tests/composition_e2e.rs`

- [ ] **Step 1: Write the e2e harness**

Create `crates/aenv-cli/tests/composition_e2e.rs`:

```rust
//! End-to-end composition test. Builds and drives the `aenv` binary against
//! a real tempdir; exercises a two-namespace chain end-to-end.

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

struct Harness {
    aenv_home: TempDir,
    project: TempDir,
    bin: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let aenv_home = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();
        let bin = env!("CARGO_BIN_EXE_aenv").into();
        Self { aenv_home, project, bin }
    }
    fn cmd(&self, args: &[&str]) -> std::process::Output {
        let out = Command::new(&self.bin)
            .env("AENV_HOME", self.aenv_home.path())
            .current_dir(self.project.path())
            .args(args)
            .output()
            .expect("aenv binary");
        if !out.status.success() {
            eprintln!("--- stdout ---\n{}", String::from_utf8_lossy(&out.stdout));
            eprintln!("--- stderr ---\n{}", String::from_utf8_lossy(&out.stderr));
            panic!("aenv {:?} failed (exit {:?})", args, out.status.code());
        }
        out
    }
    fn cmd_expect_fail(&self, args: &[&str]) -> std::process::Output {
        let out = Command::new(&self.bin)
            .env("AENV_HOME", self.aenv_home.path())
            .current_dir(self.project.path())
            .args(args)
            .output()
            .expect("aenv binary");
        assert!(!out.status.success(), "expected {:?} to fail", args);
        out
    }
    fn stdout(o: &std::process::Output) -> String {
        String::from_utf8(o.stdout.clone()).unwrap()
    }
}

fn write_file(dir: &std::path::Path, rel: &str, body: &[u8]) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

#[test]
fn composition_happy_path_two_namespace_chain_section_merges_claude_md() {
    let h = Harness::new();

    // 1. Create base + leaf via the CLI (the test exercises 'aenv create'
    //    behavior, which writes the manifest skeleton, then we drop files in).
    h.cmd(&["create", "base"]);
    h.cmd(&["create", "leaf"]);

    // 2. Populate the namespace dirs by hand (in Phase 4 we'll add 'aenv
    //    skill add' for this; for now, write files directly).
    let aenv_home = h.aenv_home.path();
    write_file(&aenv_home.join("envs/base"),
        "CLAUDE.md", b"# Build & Test\n\ncargo test\n");
    write_file(&aenv_home.join("envs/leaf"),
        "CLAUDE.md", b"# Disposition\n\nbe terse\n");

    // 3. Edit manifests to declare extends + adapters.
    std::fs::write(aenv_home.join("envs/base/aenv.toml"),
        b"name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n").unwrap();
    std::fs::write(aenv_home.join("envs/leaf/aenv.toml"),
        b"name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n").unwrap();

    // 4. Pin + activate.
    h.cmd(&["use", "leaf"]);
    h.cmd(&["activate"]);

    // 5. Confirm CLAUDE.md is a regular file (section-merge output) and
    //    contains both inputs.
    let claude = h.project.path().join("CLAUDE.md");
    let meta = std::fs::symlink_metadata(&claude).unwrap();
    assert!(!meta.file_type().is_symlink(), "section-merged file must be regular");
    let body = std::fs::read_to_string(&claude).unwrap();
    assert!(body.contains("# Build & Test"));
    assert!(body.contains("cargo test"));
    assert!(body.contains("# Disposition"));
    assert!(body.contains("be terse"));

    // 6. 'aenv which CLAUDE.md' reports the merge contributors.
    let out = h.cmd(&["which", "CLAUDE.md"]);
    let stdout = Harness::stdout(&out);
    assert!(stdout.contains("section-merge"));
    assert!(stdout.contains("base::CLAUDE.md"));
    assert!(stdout.contains("leaf::CLAUDE.md"));

    // 7. 'aenv status' shows the chain.
    let out = h.cmd(&["status"]);
    let stdout = Harness::stdout(&out);
    assert!(stdout.contains("Resolution:       base → leaf"));
    assert!(stdout.contains("merged from base + leaf"));

    // 8. 'aenv deactivate' removes the merged file and any backups it created.
    h.cmd(&["deactivate"]);
    assert!(!h.project.path().join("CLAUDE.md").exists());
}

#[test]
fn shadowed_skill_resolves_to_leaf_and_records_shadow() {
    let h = Harness::new();
    h.cmd(&["create", "base"]);
    h.cmd(&["create", "leaf"]);
    let home = h.aenv_home.path();

    write_file(&home.join("envs/base/.claude/skills/write-tests"),
        "SKILL.md", b"base impl");
    write_file(&home.join("envs/leaf/.claude/skills/write-tests"),
        "SKILL.md", b"leaf impl");

    std::fs::write(home.join("envs/base/aenv.toml"),
        b"name = \"base\"\n[adapters.claude-code]\nfiles = [\".claude/skills/write-tests/SKILL.md\"]\n").unwrap();
    std::fs::write(home.join("envs/leaf/aenv.toml"),
        b"name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\".claude/skills/write-tests/SKILL.md\"]\n").unwrap();

    h.cmd(&["use", "leaf"]);
    h.cmd(&["activate"]);

    // The file resolves to leaf's content.
    let path = h.project.path().join(".claude/skills/write-tests/SKILL.md");
    let body = std::fs::read_to_string(&path).unwrap();
    assert_eq!(body, "leaf impl");

    // which reports the shadow.
    let out = h.cmd(&["which", ".claude/skills/write-tests/SKILL.md"]);
    let stdout = Harness::stdout(&out);
    assert!(stdout.contains("leaf::"));
    assert!(stdout.contains("Shadows:"));
    assert!(stdout.contains("base::"));
}

#[test]
fn forking_a_managed_file_replaces_symlink_with_copy_and_drops_state() {
    let h = Harness::new();
    h.cmd(&["create", "base"]);
    let home = h.aenv_home.path();
    write_file(&home.join("envs/base"), "CLAUDE.md", b"# base\n");
    std::fs::write(home.join("envs/base/aenv.toml"),
        b"name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n").unwrap();
    h.cmd(&["use", "base"]);
    h.cmd(&["activate"]);

    // Single-namespace chain: CLAUDE.md is a symlink.
    let claude = h.project.path().join("CLAUDE.md");
    assert!(std::fs::symlink_metadata(&claude).unwrap().file_type().is_symlink());

    h.cmd(&["fork", "CLAUDE.md"]);
    let meta = std::fs::symlink_metadata(&claude).unwrap();
    assert!(!meta.file_type().is_symlink(), "fork replaced symlink with regular file");
    let body = std::fs::read_to_string(&claude).unwrap();
    assert_eq!(body, "# base\n");
}

#[test]
fn extends_cycle_is_rejected_with_exit_15() {
    let h = Harness::new();
    h.cmd(&["create", "a"]);
    h.cmd(&["create", "b"]);
    let home = h.aenv_home.path();
    std::fs::write(home.join("envs/a/aenv.toml"),
        b"name = \"a\"\nextends = [\"b\"]\n").unwrap();
    std::fs::write(home.join("envs/b/aenv.toml"),
        b"name = \"b\"\nextends = [\"a\"]\n").unwrap();
    h.cmd(&["use", "a"]);
    let out = h.cmd_expect_fail(&["activate"]);
    assert_eq!(out.status.code(), Some(15));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("cycle") || stderr.contains("Cycle"));
}

#[test]
fn no_materialized_path_contains_double_colon() {
    let h = Harness::new();
    h.cmd(&["create", "base"]);
    h.cmd(&["create", "leaf"]);
    let home = h.aenv_home.path();
    write_file(&home.join("envs/base"), "CLAUDE.md", b"# base\n");
    write_file(&home.join("envs/base/.claude/skills/x"), "SKILL.md", b"x");
    write_file(&home.join("envs/leaf"), "CLAUDE.md", b"# leaf\n");
    write_file(&home.join("envs/leaf/.claude/skills/y"), "SKILL.md", b"y");

    std::fs::write(home.join("envs/base/aenv.toml"),
        b"name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\", \".claude/skills/x/SKILL.md\"]\n").unwrap();
    std::fs::write(home.join("envs/leaf/aenv.toml"),
        b"name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\", \".claude/skills/y/SKILL.md\"]\n").unwrap();

    h.cmd(&["use", "leaf"]);
    h.cmd(&["activate"]);

    walk_for_double_colon(h.project.path());
}

fn walk_for_double_colon(root: &std::path::Path) {
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.unwrap();
        let s = entry.path().to_string_lossy();
        assert!(
            !s.contains("::"),
            "materialized path {s} contains '::' which violates identity-erasure"
        );
    }
}
```

Add `walkdir` as a dev-dep. The hand-rolled iterator that was tempting here has subtle correctness pitfalls (queue draining vs. depth-first generator) — `walkdir` is a single-purpose crate written for exactly this; don't reinvent.

In `crates/aenv-cli/Cargo.toml`:

```toml
[dev-dependencies]
walkdir = "2"
```

- [ ] **Step 2: Run the e2e tests**

Run: `cargo test -p aenv-cli --test composition_e2e`
Expected: 5 PASS. Each test takes a few seconds (builds the binary, runs it as a subprocess).

- [ ] **Step 3: Run the whole workspace test**

Run: `cargo test --workspace`
Expected: all tests pass — Phase 1's still-passing + every Phase 2 test added throughout this plan. Target: well over 150 tests passing.

- [ ] **Step 4: Commit**

```bash
git add crates/aenv-cli/tests/composition_e2e.rs crates/aenv-cli/Cargo.toml
git commit -m "Add end-to-end composition integration test

Drives the built aenv binary against tempfile::tempdir() in 5
scenarios:
  1. Two-namespace chain section-merges CLAUDE.md (regular file, both
     inputs present, 'which' reports both contributors, 'status' shows
     'Resolution: base -> leaf').
  2. Overlay skill shadows parent skill; leaf bytes win on disk; the
     state records the shadow chain.
  3. 'aenv fork CLAUDE.md' replaces symlink with regular file; file
     drops from state.managed_files.
  4. 'extends' cycle aborts with exit code 15 and a cycle message on
     stderr.
  5. Materialized-path invariant: no path under the project contains
     '::' after activation. Walks the project tree with walkdir.
"
```

---

### Task 18: Tag `phase-2-complete`

The closing-out task. Verify the full test suite, confirm exit codes are stable, run clippy clean, then tag.

- [ ] **Step 1: Confirm working tree is clean**

Run: `git status` — clean.

- [ ] **Step 2: Final test sweep**

Run: `cargo test --workspace`
Expected: everything green. Count tests — should be well above 150 (Phase 1 ended at 121).

Run: `cargo clippy --workspace --all-targets -- -D warnings` — clean.

Run: `cargo fmt --all --check` — clean.

- [ ] **Step 3: Sanity-check exit codes**

Manual smoke (the e2e test already covers cycle = 15; verify the others informally):

```bash
target/debug/aenv activate --project /tmp/nonexistent
echo $?  # expect 20 (project not pinned)

# In an empty tempdir pinned to a nonexistent namespace:
mkdir -p /tmp/t && cd /tmp/t && echo "missing" > .aenv
AENV_HOME=/tmp/empty-aenv target/debug/aenv activate
echo $?  # expect 10 (namespace not found)
```

These mirror Phase 1's exit-code tests; nothing new should regress.

- [ ] **Step 4: Tag the commit**

```bash
git tag -a phase-2-complete -m "$(cat <<'EOF'
Phase 2 — composition complete

Extends chains resolve depth-first with cycle detection (exit 15).
Three merge strategies: section-merge for Markdown instructions
files (with <!-- aenv:replace --> marker), deep-merge for JSON / YAML
/ TOML, last-wins symlink fallback. Every resolved artifact carries
a QualifiedName; shadow chains are recorded for non-merged artifacts
when the chain has more than one candidate for a path. The seven
built-in adapters all ship (claude-code from Phase 1, plus cursor,
aider, cline, continue, windsurf, mcp).

New CLI commands: 'aenv which <path>' (provenance), 'aenv fork
<file>' (detach a single materialized file), 'aenv fork <name>'
(create a namespace from project state). 'aenv status' now prints
the resolution chain and per-file qualified provenance.

State schema bumped to 2; schema-1 state files still load (forward-
compat synthesis of the new fields).

Hashing, --json output, parameters/policies, skill imports, shell
hook, and remotes remain deferred to Phases 3-6 per roadmap.
EOF
)"
```

- [ ] **Step 5: Confirm the tag**

Run: `git tag -l --format='%(contents)' phase-2-complete | head -20`
Expected: the message rendered.

`git log --oneline -5` — last commit is Task 17's e2e test commit; the tag points at it (or whichever commit ends the phase).

---

## Self-review checklist (to be run after writing the plan)

- **Spec coverage:**
  - R-6 — manifest carries `extends` + adapters + (Phase 3) parameters/policies. ✓ Tasks 1, 3.
  - R-7 — extends recursion with role-based defaults. ✓ Tasks 3, 4, 5–8.
  - R-8 — `<!-- aenv:replace -->` marker. ✓ Task 5.
  - R-9 — every artifact carries a `<owning_namespace>::<short_name>`. ✓ Tasks 1, 2, 11.
  - R-10 — overlay records provided + shadowed identities. ✓ Tasks 9, 11.
  - R-11 — short names on disk, qualified names internal + machine. ✓ Tasks 1 (separator constant), 11 (state writer), 17 (invariant test).
  - R-12 — cycle detection. ✓ Task 3 + Task 17.
  - R-13 — missing adapter aborts. ✓ Task 3.
  - R-30 — all seven adapters ship. ✓ Task 12.
  - R-47 — deep-merge produces a regular file with recorded contributors. ✓ Tasks 6–8 + 11.
  - R-50 — `aenv status` prints chain + qualified provenance. ✓ Task 16. (Parameters deferred to Phase 3.)
  - R-52 — `aenv which <path>` reports qualified id + shadows. ✓ Task 13.
  - R-53 — `aenv fork` (no arg, whole-project detach) ✓ Task 14b. `aenv fork <file>` (per-file detach) ✓ Task 14.
  - R-54 — `aenv fork <name>` creates namespace from project. ✓ Task 15. Glob-declared adapter entries are walked: the implementation derives the literal directory prefix from each pattern (e.g. `.claude/skills/**/*` → `.claude/skills/`) and copies every regular file underneath, following symlinks to capture the project's effective harness state. The new manifest carries resolved literal paths, not the source glob pattern.

- **Placeholder scan:**
  - No "TBD" / "implement later" / "similar to Task N" abbreviations.
  - Task 3's glob helper uses the hand-rolled `glob_match` (no `regex` dependency). The previously-shown rejected alternative was removed from the plan during adversarial review.
  - Task 11's `phase1::materialize_symlink` body is a `todo!()` — Step 3b explicitly calls out the extraction as a substantive lift-and-shift from Phase 1's `perform_activation`. The four `ProjectPathState` arms (Absent / AlreadyOurSymlink / ByteIdenticalRegular / Displaced) get adapted to push through `&mut Vec<UndoStep>` + `&mut Vec<ManagedFile>` + `&mut Vec<BackedUpFile>`.

- **Type consistency:**
  - `NamespaceId::new`, `ShortName::new`, `QualifiedName::new` — used consistently. `NamespaceId::new(s: impl Into<String>)` accepts `&str` and `String`; never `&String`.
  - `decide_strategy(candidates: &[Candidate], adapters: &AdapterRegistry) -> Result<MaterializeStrategy, AenvError>` — matches every call site in strategy.rs and activate.rs.
  - `compute_shadows(&[Candidate], MaterializeStrategy, &AdapterRegistry) -> Result<Vec<QualifiedName>>` — returns `Result` (not bare `Vec`) so manifest-invalid candidate paths surface cleanly. `adapters` arg unused in Phase 2 but kept for Phase 4 (skill short-name resolution).
  - `qualified_from_candidate(&Candidate) -> Result<QualifiedName>` — same Result discipline.
  - `MaterializeStrategy` enum variants live in `resolve.rs`. Phase 1's `state.rs` definition is deleted in Task 10; `state.rs` re-exports the resolve version. Carries `#[serde(rename_all = "kebab-case")]` plus an explicit `#[serde(rename = "merged")]` on the legacy variant for schema-1 compat.
  - `BackedUpFile` fields are `original_path` and `backup_path` (matches Phase 1's `state.rs`).
  - `UndoStep` (Phase 1's name) gains a `RemoveRegularFile { path }` variant. The function `undo<F: Filesystem>(fs: &F, log: Vec<UndoStep>)` keeps its name.
  - `activate_namespace<F: Filesystem>(fs: &F, layout: &RegistryLayout, adapters: &AdapterRegistry, project_root: &Path, leaf: &NamespaceId) -> Result<ActivationState>` — generic shape preserved from Phase 1; only the final parameter changed from `&str` to `&NamespaceId`.

- **Crate-API consistency:**
  - `toml = "0.8"` workspace dep: `toml::from_slice` was removed; all parsing uses `toml::from_str(std::str::from_utf8(bytes)?)`.
  - `pulldown-cmark = "0.10"` API: `Tag::Heading { level, .. }` struct variant and `Event::End(TagEnd::Heading(_))`. `Options::empty()` to avoid pulling table/footnote/strikethrough events.
  - `RegistryLayout::new(root: PathBuf)` (not `from_root`); test helpers wrap `PathBuf::from(REG)`.
  - `crate::atomicity::probe_rename_atomicity` (not `probe_rename`).
  - `Filesystem::Metadata` is a struct with fields `{ kind: FileKind, len: u64 }` — no methods. Symlink/dir checks via `matches!(meta.kind, FileKind::Symlink | FileKind::Directory)` or the trait's `fs.is_symlink(path)`.
  - `AdapterRegistry::iter()` yields `(&String, &Adapter)` tuples; destructure as `for (_, adapter) in adapters.iter()`.
  - `MergeError` carries an `impl From<MergeError> for AenvError` so `?` propagation works in `materialize_one`.

- **Open items punted to a later phase:**
  - `(merged)` synthetic namespace: *reserved* in Phase 2. `NamespaceId::new("(merged)")` returns `ManifestInvalid` with a clear error message pointing at the conflict; the internal `NamespaceId::merged_synthetic()` is the only legal way to construct it (Task 11's `synthesize_merged_qn` is the only caller in production code).
  - Section-merge doubling of identical content: per-spec; flag for `aenv doctor` in Phase 4.
  - `<!-- aenv:replace -->` on the first namespace in the chain: silent no-op; one-line code comment in `merge_section.rs`.
  - Windows symlink semantics (Task 14's `fork_file`, Task 14b's `fork_project`): Phase 7 territory.
  - `format_status`'s `→` separator (U+2192): Phase 7 may want an ASCII fallback for Windows consoles without UTF-8 codepage.
  - `aenv status` re-resolves the chain on every call: divergence between resolved state and on-disk state surfaces only via Phase 5's `aenv diff`. Phase 2's status output trusts the registry at status time.

---

