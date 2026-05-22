# Phase 4 — Skills Lifecycle + Instructions Defaults Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Namespaces can author skills inline (`aenv skill new`) or import them from local paths and git repositories (`aenv skill import`, with optional `--pin`). Skills materialize at activation time alongside the other adapter-managed files; imported skills are cached under `~/.aenv/cache/skills/`. `aenv skill list` enumerates the roster across namespaces. The R-24/R-25/R-26 trio for instructions-size soft limits — explicitly deferred from Phase 3 — also lands here: each built-in adapter declares its soft limit, the `instructions_budget` parameter narrows it, and the `instructions_max_chars` policy fires automatically when a manifest is silent.

**Architecture:** Skills are layered cleanly on Phase 2's resolution machinery. A namespace's `[[skills]]` table declares either `mode = "authored"` (the skill's files live under the namespace directory, materialized through the existing `resolve_namespace` candidate path) or `mode = "imported"` (the source is resolved at activation time, cached under `~/.aenv/cache/skills/<source-hash>/<ref>/`, then materialized into the project at the adapter-determined path). Imported-skill resolution is one small function per `SourceKind` (`Local`, `Git`, `Registry`-stub). Git operations shell out to `git` on PATH; an availability probe fires only when an actual git source is encountered. The R-24/R-25/R-26 work extends `Adapter` with a `soft_limits` table, declares `instructions_budget` as an adapter parameter for the six text-instruction adapters, and teaches `doctor::evaluate` to synthesize an `instructions_max_chars` policy when no manifest declares one — using `min(adapter_soft_limit, instructions_budget)` as the effective limit. State schema bumps to 4 to record imported-skill provenance (source, resolved ref, content hash); schema 3 still reads with empty skill provenance.

**Tech Stack:** Rust 1.85+ stable. New library deps: `sha2 = "0.10"` (already a transitive dep via several crates; pin in workspace). No new CLI deps. Shell-out to system `git` for clone / ls-remote / log-1 operations.

**Plan structure:** 19 tasks. T1–T3 build the skill type system (`SkillDecl`, `SkillMode`, `SourceKind`) and the `[[skills]]` manifest extension. T4 sets up the cache directory layout. T5–T7 implement the three source resolvers (local, git, registry-stub). T8 wires `required = true` semantics. T9–T10 thread skill candidates through `resolve_namespace` and bump `ActivationState` to schema 4. T11 upgrades `aenv status` to print skills. T12–T14 add the three CLI subcommands (`new`, `import`, `list`). T15–T17 close out the Phase 3 deferrals: adapter `soft_limits`, `instructions_budget` parameter declarations, and the `instructions_max_chars` auto-fire. T18 is the end-to-end CLI test (spec §5.9–§5.11 reproduced). T19 tags `phase-4-complete`. Estimated effort: 4–5 days of focused work (slightly larger than Phase 3 because of the git shell-out path).

**Repository state at start:** `main` at `phase-3-complete` (`2c253fb`) plus README + license cleanup. Workspace at 330 tests passing, `cargo fmt --check` silent, `cargo clippy --workspace --all-targets -- -D warnings` silent. `error.rs` already declares `RemoteUnreachable` (exit 14); this phase makes that variant live for git failures.

**Important Phase 0–3 invariants this plan honors:**

- `Filesystem` trait still uses `&self`. No new trait methods. Git shell-out goes through `std::process::Command` from CLI/library code — it operates on real paths under `~/.aenv/cache/`, not through the trait.
- `Filesystem::write(path, contents)` creates missing parent dirs by contract.
- All paths below the CLI layer are absolute. The library never reads `std::env::current_dir()` or `std::env::var(...)`.
- State directory is `.aenv-state/` (not `.aenv/`). Cache directory is `~/.aenv/cache/skills/` (under AENV_HOME, not under the project).
- `AenvError` variants already declared and used this phase: `RemoteUnreachable` (exit 14) for git failures, `ActivationConflict` (exit 13) for `required = true` unreachable imports. No new variants required.
- The materialized-path invariant continues to hold: no path on disk contains `::`. Skill files materialize at adapter-determined short paths.
- Tests anticipate rustfmt `max_width = 100`. Pre-format multi-arg calls.
- Backup atomicity (PRD R-45) extends to imported skills — when a skill materializes over a project file, the file is backed up first.
- Phase 3 evaluator pattern: `KEY` const + `evaluate(policy, ctx)` + free-function dispatch via `policies::builtin::dispatch`. Phase 4 preserves this contract.

**Phase 4 deliberately defers:**

- **Adapter parameter projection (R-68 second half).** The Phase 3 critique noted this was promised to Phase 4 but the projection design is adapter-specific and underspecified by the PRD (the functional spec says "claude-code knows to project `auto_invoke_subagents` into `.claude/settings.json`" without saying *where* in the JSON). Phase 4 keeps the `projects_to` declaration in adapter TOMLs but does not act on it. Defer to a dedicated mini-phase (4.5 or fold into Phase 5) once a projection schema is designed.
- **`SourceKind::Registry` resolution.** Parses but returns `"not yet implemented: registry source pending PRD §3 registry design"`. The CLI accepts `registry:<name>` and writes it to the manifest; activation produces the deferred error. Real implementation waits for the registry design.
- **`aenv skill refresh`.** Listed in the original Phase 4 scope but secondary to the main flow. Re-fetching unpinned imports happens implicitly on every activation; an explicit `refresh` command is polish that can ship later.
- **`--json` output on `aenv skill list`** — Phase 5 (with all other `--json` work).
- **`aenv edit <namespace>`** — out of scope (would require shelling to `$EDITOR`); spec §5.9 mentions it but it's a stretch goal.

---

## File structure (created or modified in this phase)

**Library (`crates/aenv-core/src/`):**

| File | Responsibility |
|---|---|
| `skills/mod.rs` | `SkillDecl`, `SkillMode { Authored \| Imported }`, `SourceKind { Local \| Git \| Registry }`, parsing of `[[skills]]` manifest tables |
| `skills/cache.rs` | `~/.aenv/cache/skills/<source-hash>/<ref>/` layout helpers; `source_hash(spec: &str) -> String` (SHA-256 hex) |
| `skills/resolve.rs` | (consolidated into `skills/mod.rs`) — `resolve_imported_skill(fs, layout, decl) -> Result<LocalResolution>` dispatches by `SourceKind` |
| `skills/local.rs` | Local-path source resolver: stat + canonicalize; no caching needed |
| `skills/git.rs` | Git source resolver: `git ls-remote` for ref discovery, `git clone --depth 1` into cache, `git rev-parse HEAD` for resolved ref. Includes `git_available()` availability probe |
| `skills/registry.rs` | Registry-source stub: returns `AenvError::ManifestInvalid("not yet implemented: registry source")` |

**Library (modified):**

- `crates/aenv-core/src/lib.rs` — re-export `pub mod skills;`
- `crates/aenv-core/src/manifest.rs` — `AenvManifest` gains `skills: Vec<SkillDecl>`
- `crates/aenv-core/src/resolve.rs` — `resolve_namespace` synthesizes candidates for skill files (authored: files under the namespace's skill directory; imported: files under the cache directory after resolution). The new candidates merge into the existing `candidates: Vec<Candidate>` flow with appropriate `adapter` attribution.
- `crates/aenv-core/src/state.rs` — `SCHEMA_VERSION` bumps to 4. `ManagedFile` gains `skill_provenance: Option<SkillProvenance>` (source, resolved_ref, resolved_hash). Schema 3 reads with `None`.
- `crates/aenv-core/src/activate/mod.rs` — pre-materialization step resolves every imported skill (calls `skills::resolve::resolve_skill`); failures abort with `AenvError::ActivationConflict` when the skill is `required = true`, otherwise `eprintln!` a warning and omit it.
- `crates/aenv-core/src/adapter.rs` — `Adapter` gains `soft_limits: BTreeMap<String, usize>` (keyed by role, e.g. `"instructions" = 5000`).
- `crates/aenv-core/src/policies/builtin/instructions_max_chars.rs` — when evaluating, factor in `effective_limit = min(policy.value, adapter_soft_limit_for_role, instructions_budget_parameter)` per R-26.
- `crates/aenv-core/src/doctor.rs` — `evaluate()` synthesizes an `instructions_max_chars` policy when no manifest declares one AND at least one adapter has a `soft_limits.instructions` entry. Synthesized policy is attributed to the leaf namespace with `synthesized: true`.
- `crates/aenv-core/src/adapters_builtin/{claude_code, cursor, cline, continue_, aider, windsurf, mcp}.toml` — claude_code/cursor/cline/continue/aider declare `[soft_limits] instructions = 5000`; windsurf declares `instructions = 6000`. The six text-instruction adapters add `[[parameters]] name = "instructions_budget" type = "integer"`.

**Binary (`crates/aenv-cli/src/`):**

| File | Responsibility |
|---|---|
| `cmd/skill/mod.rs` | `pub mod new; pub mod import; pub mod list;` + subcommand action enum |
| `cmd/skill/new.rs` | `aenv skill new <name> --ns <ns> [--adapter <a>]` — scaffold authored skill |
| `cmd/skill/import.rs` | `aenv skill import <source> --ns <ns> [--pin <ref>]` — add imported entry, resolve source, optionally pin |
| `cmd/skill/list.rs` | `aenv skill list [--ns <ns>]` — text table |
| `main.rs` (modify) | Add `Skill { action }` subcommand to clap |
| `cmd/mod.rs` (modify) | `pub mod skill;` |
| `cmd/status.rs` (modify) | Append a "Skills:" section listing each managed file with `skill_provenance`, showing source + resolved ref |

**Tests (new):**

- `crates/aenv-core/tests/skill_decl.rs` — manifest `[[skills]]` parses; authored/imported discrimination; source-string parsing into `SourceKind`
- `crates/aenv-core/tests/skill_cache.rs` — `source_hash` is deterministic + collision-resistant; cache path derivation
- `crates/aenv-core/tests/skill_resolve_local.rs` — local-path source: exists + missing
- `crates/aenv-core/tests/skill_resolve_git.rs` — git source resolution (uses a tempdir + `git init --bare` fixture; only runs when `git` is on PATH; otherwise the test prints a one-line skip)
- `crates/aenv-core/tests/skill_required.rs` — required + missing → activation aborts (exit 13); required + reachable → activation succeeds
- `crates/aenv-core/tests/activate_authored_skill.rs` — manifest declares an authored skill; activation materializes the skill files at the project path
- `crates/aenv-core/tests/activate_imported_skill_local.rs` — manifest declares a local-path import; activation materializes from the resolved source
- `crates/aenv-core/tests/state_schema_4.rs` — schema 3 reads cleanly (no skill provenance); schema 4 round-trip preserves provenance
- `crates/aenv-core/tests/doctor_auto_instructions_limit.rs` — adapter declares `soft_limits.instructions = 5000`; manifest declares no policy; namespace has an oversized CLAUDE.md; doctor reports a violation
- `crates/aenv-core/tests/doctor_instructions_budget_narrows.rs` — adapter limit = 5000, manifest `instructions_budget = 3000`, CLAUDE.md is 4000 chars; doctor reports a violation (because effective = 3000)
- `crates/aenv-cli/tests/skill_new_e2e.rs` — `aenv skill new` creates SKILL.md + appends to manifest
- `crates/aenv-cli/tests/skill_import_local_e2e.rs` — `aenv skill import <local-path>` works
- `crates/aenv-cli/tests/skill_import_git_e2e.rs` — `aenv skill import git+file://...` works (uses local bare repo fixture; skips if git not on PATH)
- `crates/aenv-cli/tests/skill_list_e2e.rs` — text-table output covers mode/source/pin

---

## Glossary (for the implementer)

- **SkillDecl** — one entry in a manifest's `[[skills]]` array: `{ name, mode, adapter?, source?, ref?, required? }`. Adapter defaults to the namespace's single adapter if there's exactly one; required defaults to `false`.
- **SkillMode** — `Authored` or `Imported`. Discriminates how the resolver finds the skill's files.
- **SourceKind** — parsed shape of an import `source` string. `Local("/abs/path")`, `Git { url, ref_spec }` (where the URL is everything before `#ref`, the ref_spec is everything after), or `Registry(name)` (Phase 4 stubbed).
- **LocalResolution** — `{ source_path: PathBuf, resolved_ref: Option<String>, resolved_hash: String }`. Returned by every source resolver (local, git, registry-stub). `source_path` is the absolute directory containing the resolved skill files; `resolved_ref` is the git SHA for git sources and `None` for local; `resolved_hash` is `"sha256:<hex>"` of the SKILL.md content. The qualified name is constructed at the call site (`<namespace>::<skill-name>`) when this resolution is wired into a `Candidate`.
- **Adapter-determined path** — for the claude-code adapter, skills live at `.claude/skills/<skill-name>/`. The convention is hard-coded per adapter in the adapter TOML's `skills_dir` field (added this phase; defaults to `.claude/skills`).
- **Skill content layout** — a skill is a directory with at minimum a `SKILL.md`. Other files (assets, sub-prompts) under the skill directory are also materialized.
- **Cache directory** — `~/.aenv/cache/skills/<source-hash>/<ref>/<files...>`. `<source-hash>` is the first 16 hex chars of SHA-256(source-string). `<ref>` is the resolved git ref or "head" for unpinned.
- **soft_limits** — adapter-declared character limits per role. Phase 4 only uses `"instructions"`. Effective limit when evaluating `instructions_max_chars` is `min(adapter.soft_limits["instructions"], parameters.instructions_budget, policy_value_if_declared)`.
- **Synthesized policy** — a `ResolvedPolicy` constructed at doctor-evaluation time (not from manifest declaration) to fire `instructions_max_chars` even when no manifest declared the policy. Provenance shows `(from <leaf>, auto-injected from adapter default)`.

---

### Task 1: `SkillDecl` + `SkillMode` types

Pure types. No filesystem, no async. Owns the in-memory shape of a `[[skills]]` entry.

**Files:**
- Create: `crates/aenv-core/src/skills/mod.rs`
- Modify: `crates/aenv-core/src/lib.rs` (add `pub mod skills;`)
- Test: `crates/aenv-core/tests/skill_decl_types.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/skill_decl_types.rs`:

```rust
use aenv_core::skills::{SkillDecl, SkillMode};

#[test]
fn authored_decl_shape() {
    let s = SkillDecl {
        name: "write-tests".into(),
        mode: SkillMode::Authored,
        adapter: Some("claude-code".into()),
        source: None,
        ref_: None,
        required: false,
    };
    assert_eq!(s.name, "write-tests");
    assert!(matches!(s.mode, SkillMode::Authored));
    assert_eq!(s.adapter.as_deref(), Some("claude-code"));
    assert!(s.source.is_none());
}

#[test]
fn imported_decl_shape() {
    let s = SkillDecl {
        name: "match-conventions".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("git+https://github.com/acme/aenv-skills.git#match-conventions".into()),
        ref_: Some("v1.2.0".into()),
        required: true,
    };
    assert!(matches!(s.mode, SkillMode::Imported));
    assert!(s.required);
    assert_eq!(s.ref_.as_deref(), Some("v1.2.0"));
}

#[test]
fn skill_mode_round_trips_via_serde() {
    let authored = SkillMode::Authored;
    let json = serde_json::to_string(&authored).unwrap();
    assert_eq!(json, "\"authored\"");
    let back: SkillMode = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, SkillMode::Authored));

    let imported = SkillMode::Imported;
    let json = serde_json::to_string(&imported).unwrap();
    assert_eq!(json, "\"imported\"");
    let back: SkillMode = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, SkillMode::Imported));
}

#[test]
fn skill_decl_round_trips_via_toml() {
    let s = SkillDecl {
        name: "x".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/local/path".into()),
        ref_: None,
        required: false,
    };
    let rendered = toml::to_string(&s).unwrap();
    let back: SkillDecl = toml::from_str(&rendered).unwrap();
    assert_eq!(s, back);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_decl_types 2>&1 | tail -10`
Expected: FAIL — `aenv_core::skills` module does not exist.

- [ ] **Step 3: Implement the types**

Create `crates/aenv-core/src/skills/mod.rs`:

```rust
//! Skill content model for namespaces.
//!
//! Phase 4 introduces two flavors of skill: *authored* skills whose files
//! live under the namespace's own directory (and materialize through the
//! standard adapter-files path), and *imported* skills whose `source` is
//! resolved at activation time (local path or git URL, optionally pinned).
//! `SourceKind` parsing lives in `skills::source` (Task 2); the `SkillDecl`
//! struct here is the wire shape that lands in `aenv.toml`'s `[[skills]]`
//! table.

use serde::{Deserialize, Serialize};

/// One `[[skills]]` entry in a manifest.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillDecl {
    /// Skill name. Becomes the directory name under the adapter's `skills_dir`
    /// at materialization time. Must be unique within a namespace.
    pub name: String,
    /// Whether the skill's files live in the namespace tree (`Authored`) or
    /// are fetched at activation time from a `source` (`Imported`).
    pub mode: SkillMode,
    /// Which adapter manages this skill. Optional when the namespace declares
    /// exactly one adapter (then defaults to that adapter at resolution time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// Required for `mode = "imported"`. The form of the source determines
    /// how it's resolved (see `SourceKind` in `skills::source`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Optional pinned ref for `mode = "imported"`. When omitted, the
    /// importer resolves to head at each activation and records the resolved
    /// ref in `state.json`.
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    /// When `true`, an unreachable import fails activation (R-22). Default
    /// `false` means: report the failure, omit this skill, continue.
    #[serde(default)]
    pub required: bool,
}

/// Whether a skill's files live in the namespace tree or come from outside.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillMode {
    /// Skill files live under the namespace's own directory.
    Authored,
    /// Skill files come from a resolved `source` at activation time.
    Imported,
}
```

Add to `crates/aenv-core/src/lib.rs`:

```rust
pub mod skills;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_decl_types 2>&1 | tail -10`
Expected: PASS — 4 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/skills/mod.rs crates/aenv-core/src/lib.rs crates/aenv-core/tests/skill_decl_types.rs
git commit -m "Add SkillDecl + SkillMode types"
```

---

### Task 2: `SourceKind` parser

A source-string discriminator. `git+https://...#ref` → `Git`. `/abs/path` or `~/path` → `Local`. `registry:<name>` → `Registry`. Anything else: `ManifestInvalid`.

**Files:**
- Create: `crates/aenv-core/src/skills/source.rs`
- Modify: `crates/aenv-core/src/skills/mod.rs` (add `pub mod source;`)
- Test: `crates/aenv-core/tests/skill_source_kind.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/skill_source_kind.rs`:

```rust
use aenv_core::skills::source::SourceKind;

#[test]
fn parses_local_absolute() {
    let s = SourceKind::parse("/home/user/skills/foo").unwrap();
    match s {
        SourceKind::Local(p) => assert_eq!(p.to_string_lossy(), "/home/user/skills/foo"),
        _ => panic!("expected Local, got {s:?}"),
    }
}

#[test]
fn parses_local_tilde_unexpanded() {
    // Tilde expansion is the CLI layer's responsibility; the parser keeps the
    // literal so callers can normalize it.
    let s = SourceKind::parse("~/team-skills/foo").unwrap();
    match s {
        SourceKind::Local(p) => assert_eq!(p.to_string_lossy(), "~/team-skills/foo"),
        _ => panic!("expected Local, got {s:?}"),
    }
}

#[test]
fn parses_git_url_with_fragment_ref() {
    let s = SourceKind::parse("git+https://github.com/acme/aenv-skills.git#match-conventions")
        .unwrap();
    match s {
        SourceKind::Git { url, ref_spec } => {
            assert_eq!(url, "https://github.com/acme/aenv-skills.git");
            assert_eq!(ref_spec.as_deref(), Some("match-conventions"));
        }
        _ => panic!("expected Git, got {s:?}"),
    }
}

#[test]
fn parses_git_url_without_fragment() {
    let s = SourceKind::parse("git+https://github.com/acme/aenv-skills.git").unwrap();
    match s {
        SourceKind::Git { url, ref_spec } => {
            assert_eq!(url, "https://github.com/acme/aenv-skills.git");
            assert!(ref_spec.is_none());
        }
        _ => panic!("expected Git, got {s:?}"),
    }
}

#[test]
fn parses_registry_source() {
    let s = SourceKind::parse("registry:cite-evidence").unwrap();
    match s {
        SourceKind::Registry(name) => assert_eq!(name, "cite-evidence"),
        _ => panic!("expected Registry, got {s:?}"),
    }
}

#[test]
fn rejects_unknown_prefix() {
    let err = SourceKind::parse("https://example.com/skill.zip").unwrap_err();
    assert!(err.to_string().contains("source"));
}

#[test]
fn rejects_empty() {
    let err = SourceKind::parse("").unwrap_err();
    assert!(err.to_string().contains("empty") || err.to_string().contains("source"));
}

#[test]
fn rejects_relative_local_path() {
    // We require absolute (or tilde-prefixed) paths to avoid ambiguity with
    // the registry shorthand.
    let err = SourceKind::parse("./my-skill").unwrap_err();
    assert!(err.to_string().contains("relative") || err.to_string().contains("absolute"));
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_source_kind 2>&1 | tail -10`
Expected: FAIL — `aenv_core::skills::source` does not exist.

- [ ] **Step 3: Implement the parser**

Create `crates/aenv-core/src/skills/source.rs`:

```rust
//! Discriminate a skill `source` string by its form.
//!
//! Three shapes are recognized:
//!
//! * `/abs/path` or `~/path` → `Local`. Tilde expansion is the CLI's job.
//! * `git+<scheme>://...#<ref>` → `Git`. The `#<ref>` fragment is optional.
//! * `registry:<name>` → `Registry`. Phase 4 stubs resolution.
//!
//! Anything else is `ManifestInvalid` with a hint.

use crate::error::{AenvError, Result};
use std::path::PathBuf;

/// Parsed form of a skill source.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SourceKind {
    /// Filesystem path. May be absolute or tilde-prefixed.
    Local(PathBuf),
    /// Git URL (with `git+` prefix stripped) and optional `#ref`.
    Git {
        /// URL after the `git+` prefix, before any `#ref` fragment.
        url: String,
        /// Anything after a `#` fragment marker. `None` if not specified.
        ref_spec: Option<String>,
    },
    /// Forward-compat registry shorthand. Phase 4 stubs resolution.
    Registry(String),
}

impl SourceKind {
    /// Parse a source string into one of the three known shapes.
    pub fn parse(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(AenvError::ManifestInvalid(
                "skill source is empty".to_string(),
            ));
        }
        if let Some(rest) = s.strip_prefix("git+") {
            let (url, ref_spec) = match rest.split_once('#') {
                Some((u, r)) => (u.to_string(), Some(r.to_string())),
                None => (rest.to_string(), None),
            };
            return Ok(SourceKind::Git { url, ref_spec });
        }
        if let Some(name) = s.strip_prefix("registry:") {
            if name.is_empty() {
                return Err(AenvError::ManifestInvalid(
                    "registry source has empty name".to_string(),
                ));
            }
            return Ok(SourceKind::Registry(name.to_string()));
        }
        if s.starts_with('/') || s.starts_with('~') {
            return Ok(SourceKind::Local(PathBuf::from(s)));
        }
        Err(AenvError::ManifestInvalid(format!(
            "skill source '{s}' is not recognized as 'git+<url>[#ref]', \
             'registry:<name>', or an absolute / tilde-prefixed path. \
             Relative paths are rejected to avoid ambiguity."
        )))
    }
}
```

Add to `crates/aenv-core/src/skills/mod.rs`:

```rust
pub mod source;
```

- [ ] **Step 4: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_source_kind 2>&1 | tail -10`
Expected: PASS — 8 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/skills/source.rs crates/aenv-core/src/skills/mod.rs crates/aenv-core/tests/skill_source_kind.rs
git commit -m "Add SourceKind parser for skill source strings"
```

---

### Task 3: Extend `AenvManifest` with `[[skills]]`

Manifests can now carry a `skills` array. Same two-stage parse pattern as Phase 3's `[parameters]` / `[policies]`: parse raw, then validate per-entry. Validate: every imported skill MUST have a `source`; every authored skill MUST NOT have a `source` (it's noise).

**Files:**
- Modify: `crates/aenv-core/src/manifest.rs`
- Test: `crates/aenv-core/tests/manifest_skills.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/manifest_skills.rs`:

```rust
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::SkillMode;

#[test]
fn parses_authored_skill() {
    let toml = r#"
name = "experiments"

[[skills]]
name = "compare-approaches"
mode = "authored"
adapter = "claude-code"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills.len(), 1);
    assert_eq!(m.skills[0].name, "compare-approaches");
    assert!(matches!(m.skills[0].mode, SkillMode::Authored));
    assert_eq!(m.skills[0].adapter.as_deref(), Some("claude-code"));
}

#[test]
fn parses_imported_skill_pinned() {
    let toml = r#"
name = "detailed-execution"

[[skills]]
name = "match-conventions"
mode = "imported"
adapter = "claude-code"
source = "git+https://github.com/acme/aenv-skills.git#match-conventions"
ref = "v1.2.0"
required = true
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills.len(), 1);
    assert!(matches!(m.skills[0].mode, SkillMode::Imported));
    assert!(m.skills[0].required);
    assert_eq!(m.skills[0].ref_.as_deref(), Some("v1.2.0"));
}

#[test]
fn parses_multiple_skills() {
    let toml = r#"
name = "x"

[[skills]]
name = "a"
mode = "authored"

[[skills]]
name = "b"
mode = "imported"
source = "/local/path/b"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills.len(), 2);
    assert_eq!(m.skills[0].name, "a");
    assert_eq!(m.skills[1].name, "b");
}

#[test]
fn missing_block_is_empty_vec() {
    let toml = r#"name = "x""#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert!(m.skills.is_empty());
}

#[test]
fn rejects_imported_without_source() {
    let toml = r#"
name = "x"

[[skills]]
name = "needs-source"
mode = "imported"
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("needs-source"));
    assert!(err.to_string().contains("source"));
}

#[test]
fn rejects_authored_with_source() {
    let toml = r#"
name = "x"

[[skills]]
name = "stray"
mode = "authored"
source = "/somewhere"
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("stray"));
    assert!(err.to_string().contains("source"));
}

#[test]
fn rejects_duplicate_skill_names() {
    let toml = r#"
name = "x"

[[skills]]
name = "dup"
mode = "authored"

[[skills]]
name = "dup"
mode = "authored"
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("dup"));
}

#[test]
fn roundtrip_preserves_skills() {
    let toml = r#"
name = "x"

[[skills]]
name = "a"
mode = "imported"
source = "/p"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let rendered = m.to_toml();
    let m2 = AenvManifest::from_toml(&rendered).unwrap();
    assert_eq!(m, m2);
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test manifest_skills 2>&1 | tail -10`
Expected: FAIL — `AenvManifest::skills` field does not exist.

- [ ] **Step 3: Extend `AenvManifest`**

In `crates/aenv-core/src/manifest.rs`, update the imports, the struct, and `from_toml` / `default_for`:

```rust
use crate::error::{AenvError, Result};
use crate::parameters::ParameterValue;
use crate::policies::{policy_table_from_toml, PolicyDecl};
use crate::skills::{SkillDecl, SkillMode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AenvManifest {
    pub name: String,
    #[serde(default)]
    pub extends: Vec<String>,
    #[serde(default)]
    pub adapters: BTreeMap<String, AdapterEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub policies: BTreeMap<String, PolicyDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillDecl>,
}

impl AenvManifest {
    pub fn from_toml(input: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            #[serde(default)]
            extends: Vec<String>,
            #[serde(default)]
            adapters: BTreeMap<String, AdapterEntry>,
            #[serde(default)]
            parameters: BTreeMap<String, toml::Value>,
            #[serde(default)]
            policies: BTreeMap<String, toml::Value>,
            #[serde(default)]
            skills: Vec<SkillDecl>,
        }
        let raw: Raw =
            toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;

        let mut parameters: BTreeMap<String, ParameterValue> = BTreeMap::new();
        for (k, v) in &raw.parameters {
            let pv = ParameterValue::from_toml_value(v).map_err(|e| match e {
                AenvError::ManifestInvalid(reason) => {
                    AenvError::ManifestInvalid(format!("parameter '{k}': {reason}"))
                }
                other => other,
            })?;
            parameters.insert(k.clone(), pv);
        }
        let policies = policy_table_from_toml(&raw.policies)?;
        validate_skills(&raw.skills)?;

        Ok(AenvManifest {
            name: raw.name,
            extends: raw.extends,
            adapters: raw.adapters,
            parameters,
            policies,
            skills: raw.skills,
        })
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("AenvManifest serialization is infallible")
    }

    pub fn default_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            extends: Vec::new(),
            adapters: BTreeMap::new(),
            parameters: BTreeMap::new(),
            policies: BTreeMap::new(),
            skills: Vec::new(),
        }
    }
}

fn validate_skills(skills: &[SkillDecl]) -> Result<()> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for s in skills {
        if !seen.insert(s.name.as_str()) {
            return Err(AenvError::ManifestInvalid(format!(
                "skill '{}' declared more than once",
                s.name
            )));
        }
        match s.mode {
            SkillMode::Authored => {
                if s.source.is_some() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "skill '{}' is authored but declares a source; \
                         remove `source` or change mode to 'imported'",
                        s.name
                    )));
                }
            }
            SkillMode::Imported => {
                if s.source.is_none() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "skill '{}' is imported but declares no source",
                        s.name
                    )));
                }
            }
        }
    }
    Ok(())
}
```

You also need to update any other struct-literal sites for `AenvManifest`. Find them with:

```bash
grep -rn "AenvManifest {" crates/
```

Add `skills: Vec::new()` to each. Likely sites: `crates/aenv-core/src/namespace.rs`.

- [ ] **Step 4: Run new + existing tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test manifest_skills 2>&1 | tail -10`
Expected: PASS — 8 tests passed.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/manifest.rs crates/aenv-core/tests/manifest_skills.rs
# Plus any struct-literal sites you updated.
git add -u
git commit -m "Extend AenvManifest with [[skills]] table"
```

---

### Task 4: Skill cache layout

The cache directory lives under `AENV_HOME/cache/skills/<source-hash>/<ref>/`. `<source-hash>` is the first 16 hex chars of SHA-256(source-string) — collision-resistant enough that two different sources won't share a directory. `<ref>` is the resolved git ref or the literal `"head"` for unpinned sources at first resolution.

We need a new workspace dependency: `sha2`.

**Files:**
- Modify: `Cargo.toml` (workspace) — add `sha2 = "0.10"` to `[workspace.dependencies]`
- Modify: `crates/aenv-core/Cargo.toml` — add `sha2 = { workspace = true }`
- Create: `crates/aenv-core/src/skills/cache.rs`
- Modify: `crates/aenv-core/src/skills/mod.rs` (add `pub mod cache;`)
- Modify: `crates/aenv-core/src/home.rs` — add `cache_dir()` and `skills_cache_dir()` helpers
- Test: `crates/aenv-core/tests/skill_cache.rs`

- [ ] **Step 1: Add the workspace dependency**

In the workspace root `Cargo.toml`, add to `[workspace.dependencies]`:

```toml
sha2 = "0.10"
```

In `crates/aenv-core/Cargo.toml`, add to `[dependencies]`:

```toml
sha2 = { workspace = true }
```

Verify it compiles:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo build -p aenv-core 2>&1 | tail -5
```

Expected: clean build (sha2 downloads + compiles on first run; subsequent runs are cached).

- [ ] **Step 2: Write the failing test**

Create `crates/aenv-core/tests/skill_cache.rs`:

```rust
use aenv_core::home::RegistryLayout;
use aenv_core::skills::cache::{source_hash, skill_cache_path};
use std::path::PathBuf;

#[test]
fn source_hash_is_deterministic() {
    let h1 = source_hash("git+https://example.com/foo.git#main");
    let h2 = source_hash("git+https://example.com/foo.git#main");
    assert_eq!(h1, h2);
}

#[test]
fn source_hash_differs_for_different_sources() {
    let h1 = source_hash("git+https://example.com/foo.git#main");
    let h2 = source_hash("git+https://example.com/foo.git#feature");
    assert_ne!(h1, h2);
}

#[test]
fn source_hash_is_16_hex_chars() {
    let h = source_hash("anything");
    assert_eq!(h.len(), 16);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn cache_path_for_pinned_ref() {
    let layout = RegistryLayout::new(PathBuf::from("/home/u/.aenv"));
    let p = skill_cache_path(&layout, "git+https://example.com/foo.git", "v1.2.0");
    let hash = source_hash("git+https://example.com/foo.git");
    assert_eq!(
        p,
        PathBuf::from(format!("/home/u/.aenv/cache/skills/{hash}/v1.2.0"))
    );
}

#[test]
fn cache_path_for_unpinned_head() {
    let layout = RegistryLayout::new(PathBuf::from("/home/u/.aenv"));
    let p = skill_cache_path(&layout, "/local/path", "head");
    let hash = source_hash("/local/path");
    assert_eq!(
        p,
        PathBuf::from(format!("/home/u/.aenv/cache/skills/{hash}/head"))
    );
}
```

- [ ] **Step 3: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_cache 2>&1 | tail -10`
Expected: FAIL — `aenv_core::skills::cache` does not exist; `RegistryLayout::cache_dir` does not exist.

- [ ] **Step 4: Extend `home.rs`**

In `crates/aenv-core/src/home.rs`, add two methods to `RegistryLayout`:

```rust
    /// The `cache/` subdirectory holding fetched skill content and other
    /// transient caches that aenv manages.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// The `cache/skills/` subdirectory.
    pub fn skills_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("skills")
    }
```

- [ ] **Step 5: Create `skills/cache.rs`**

Create `crates/aenv-core/src/skills/cache.rs`:

```rust
//! Cache directory layout for fetched skills.
//!
//! Fetched (imported) skills are cached under
//! `AENV_HOME/cache/skills/<source-hash>/<ref>/<files...>`.
//! `<source-hash>` is the first 16 hex chars of SHA-256(source-string) —
//! collision-resistant enough that two different sources will never share
//! a directory in practice. `<ref>` is the resolved git ref or the literal
//! `"head"` for unpinned sources at first resolution.

use crate::home::RegistryLayout;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Stable 16-hex-char hash of a source string.
pub fn source_hash(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let bytes = hasher.finalize();
    let hex: String = bytes.iter().take(8).map(|b| format!("{b:02x}")).collect();
    hex
}

/// Absolute path to the cached content for a (source, ref) pair.
///
/// Does NOT create the directory; callers materialize as needed.
pub fn skill_cache_path(layout: &RegistryLayout, source: &str, ref_label: &str) -> PathBuf {
    layout
        .skills_cache_dir()
        .join(source_hash(source))
        .join(ref_label)
}
```

Add to `crates/aenv-core/src/skills/mod.rs`:

```rust
pub mod cache;
```

- [ ] **Step 6: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_cache 2>&1 | tail -10`
Expected: PASS — 5 tests passed.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/aenv-core/Cargo.toml crates/aenv-core/src/skills/cache.rs crates/aenv-core/src/skills/mod.rs crates/aenv-core/src/home.rs crates/aenv-core/tests/skill_cache.rs Cargo.lock
git commit -m "Add skill cache layout (source-hash + ref path scheme)"
```

---

### Task 5: Local-path skill source resolver

A skill `source` of the form `/abs/path` or `~/path` resolves by stat-ing the path and reading the SKILL.md to compute a content hash. No caching — the local files ARE the cache.

**Files:**
- Create: `crates/aenv-core/src/skills/local.rs`
- Modify: `crates/aenv-core/src/skills/mod.rs` (add `pub mod local;` + the `ResolvedSkill` type)
- Test: `crates/aenv-core/tests/skill_resolve_local.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/skill_resolve_local.rs`:

```rust
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::skills::local::resolve_local;
use std::path::PathBuf;

#[test]
fn resolves_when_skill_md_exists() {
    let fs = MockFilesystem::new();
    fs.create_dir_all(&PathBuf::from("/local/my-skill")).unwrap();
    fs.write(&PathBuf::from("/local/my-skill/SKILL.md"), b"---\nname: x\n---\nbody")
        .unwrap();

    let r = resolve_local(&fs, &PathBuf::from("/local/my-skill"), "my-skill").unwrap();
    assert_eq!(r.source_path, PathBuf::from("/local/my-skill"));
    assert!(r.resolved_ref.is_none());
    // Same bytes always produce the same hash.
    assert!(r.resolved_hash.starts_with("sha256:"));
    assert!(r.resolved_hash.len() > "sha256:".len());
}

#[test]
fn hash_changes_with_content() {
    let fs = MockFilesystem::new();
    fs.write(&PathBuf::from("/a/SKILL.md"), b"first").unwrap();
    fs.write(&PathBuf::from("/b/SKILL.md"), b"second").unwrap();
    let r1 = resolve_local(&fs, &PathBuf::from("/a"), "x").unwrap();
    let r2 = resolve_local(&fs, &PathBuf::from("/b"), "x").unwrap();
    assert_ne!(r1.resolved_hash, r2.resolved_hash);
}

#[test]
fn errors_when_skill_md_missing() {
    let fs = MockFilesystem::new();
    fs.create_dir_all(&PathBuf::from("/empty/dir")).unwrap();
    let err = resolve_local(&fs, &PathBuf::from("/empty/dir"), "x").unwrap_err();
    assert!(err.to_string().contains("SKILL.md"));
}

#[test]
fn errors_when_directory_missing() {
    let fs = MockFilesystem::new();
    let err = resolve_local(&fs, &PathBuf::from("/does/not/exist"), "x").unwrap_err();
    assert!(
        err.to_string().contains("does not exist")
            || err.to_string().contains("not found"),
        "msg = {err}"
    );
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_resolve_local 2>&1 | tail -10`
Expected: FAIL — `aenv_core::skills::local::resolve_local` does not exist.

- [ ] **Step 3: Add the `local` submodule pointer**

Add to `crates/aenv-core/src/skills/mod.rs`:

```rust
pub mod local;
```

The shared resolution shape (`LocalResolution`) is defined inside `local.rs` itself in Step 4 — git and registry resolvers re-use the same type for symmetry.

- [ ] **Step 4: Implement `local.rs`**

Create `crates/aenv-core/src/skills/local.rs`:

```rust
//! Resolve a local-path skill source.
//!
//! Local sources don't need a cache — the path on disk IS the source. We
//! only verify that `SKILL.md` exists under the given directory and compute
//! a content hash for state-file provenance.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Result of resolving a local-path skill source.
pub struct LocalResolution {
    /// Absolute source directory.
    pub source_path: std::path::PathBuf,
    /// `None` — local sources have no ref.
    pub resolved_ref: Option<String>,
    /// `"sha256:<hex>"` of the SKILL.md body.
    pub resolved_hash: String,
}

/// Validate that `<source_dir>/SKILL.md` exists and hash its bytes.
pub fn resolve_local<F: Filesystem>(
    fs: &F,
    source_dir: &Path,
    _skill_name: &str,
) -> Result<LocalResolution> {
    if !fs.exists(source_dir)? {
        return Err(AenvError::ManifestInvalid(format!(
            "local skill source directory does not exist: {}",
            source_dir.display()
        )));
    }
    let skill_md = source_dir.join("SKILL.md");
    if !fs.exists(&skill_md)? {
        return Err(AenvError::ManifestInvalid(format!(
            "local skill source {} has no SKILL.md",
            source_dir.display()
        )));
    }
    let bytes = fs.read(&skill_md)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(LocalResolution {
        source_path: source_dir.to_path_buf(),
        resolved_ref: None,
        resolved_hash: format!("sha256:{hex}"),
    })
}
```

- [ ] **Step 5: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_resolve_local 2>&1 | tail -10`
Expected: PASS — 4 tests passed.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/skills/local.rs crates/aenv-core/src/skills/mod.rs crates/aenv-core/tests/skill_resolve_local.rs
git commit -m "Add local-path skill source resolver"
```

---

### Task 6: Git wrapper + availability probe

A thin shell-out layer for `git ls-remote`, `git clone --depth 1`, `git rev-parse HEAD`. Lives in the library because activation needs it, but uses `std::process::Command` directly (not through the `Filesystem` trait — git operates on real disk). Includes a `git_available()` probe that runs `git --version` once and caches the result for the process.

The wrapper exports three functions:
- `git_available() -> bool` — probe, run lazily.
- `git_resolve_ref(url: &str, ref_spec: Option<&str>) -> Result<String>` — uses `ls-remote` to find a commit SHA for an optional ref (branch/tag). If `ref_spec` is `None`, defaults to HEAD.
- `git_clone(url: &str, ref_spec: Option<&str>, dest: &Path) -> Result<String>` — shallow clone, checks out the ref, returns the resolved commit SHA.

**Files:**
- Create: `crates/aenv-core/src/skills/git.rs`
- Modify: `crates/aenv-core/src/skills/mod.rs` (add `pub mod git;`)
- Test: `crates/aenv-core/tests/skill_git_probe.rs`

These tests touch real git via `std::process::Command`. They GATE on `git` being on PATH: if `git --version` exits nonzero, the test prints a one-line skip notice and returns Ok (without asserting). On CI we'll have git available; locally we trust the developer environment.

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/skill_git_probe.rs`:

```rust
use aenv_core::skills::git::{git_available, git_clone, git_resolve_ref};
use std::process::Command;
use tempfile::tempdir;

fn skip_unless_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn git_available_returns_true_when_on_path() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    assert!(git_available());
}

#[test]
fn ls_remote_resolves_local_bare_repo_head() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = tempdir().unwrap();
    let bare_path = bare.path();
    // Initialize a bare repo with one commit.
    Command::new("git")
        .args(["init", "--bare"])
        .arg(bare_path)
        .status()
        .unwrap();
    let work = tempdir().unwrap();
    let work_path = work.path();
    Command::new("git")
        .args(["clone"])
        .arg(bare_path)
        .arg(work_path)
        .status()
        .unwrap();
    std::fs::write(work_path.join("README.md"), b"hi").unwrap();
    Command::new("git").current_dir(work_path).args(["add", "."]).status().unwrap();
    Command::new("git")
        .current_dir(work_path)
        .args([
            "-c", "user.email=t@e", "-c", "user.name=t",
            "commit", "-m", "init",
        ])
        .status()
        .unwrap();
    Command::new("git").current_dir(work_path).args(["push", "origin", "HEAD:master"]).status().unwrap();

    let url = format!("file://{}", bare_path.display());
    let sha = git_resolve_ref(&url, None).unwrap();
    assert_eq!(sha.len(), 40, "expected full SHA, got {sha:?}");
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn clone_to_destination_returns_resolved_sha() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    // Set up a tiny bare repo as in the ls_remote test.
    let bare = tempdir().unwrap();
    let bare_path = bare.path();
    Command::new("git").args(["init", "--bare"]).arg(bare_path).status().unwrap();
    let work = tempdir().unwrap();
    let work_path = work.path();
    Command::new("git").args(["clone"]).arg(bare_path).arg(work_path).status().unwrap();
    std::fs::write(work_path.join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    Command::new("git").current_dir(work_path).args(["add", "."]).status().unwrap();
    Command::new("git")
        .current_dir(work_path)
        .args(["-c", "user.email=t@e", "-c", "user.name=t", "commit", "-m", "init"])
        .status()
        .unwrap();
    Command::new("git").current_dir(work_path).args(["push", "origin", "HEAD:master"]).status().unwrap();

    let url = format!("file://{}", bare_path.display());
    let dest = tempdir().unwrap();
    let sha = git_clone(&url, None, dest.path()).unwrap();
    assert_eq!(sha.len(), 40);
    assert!(dest.path().join("SKILL.md").exists());
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_git_probe 2>&1 | tail -10`
Expected: FAIL — `aenv_core::skills::git` does not exist.

- [ ] **Step 3: Implement `git.rs`**

Create `crates/aenv-core/src/skills/git.rs`:

```rust
//! Shell-out wrapper around system `git`.
//!
//! Used by the imported-skill resolver. Tests should gate on `git_available()`
//! so they skip cleanly when git isn't on PATH.
//!
//! Why shell out rather than use libgit2: `git2`'s dependency footprint is
//! large (libgit2 + libssh2 + zlib + libssl), and `aenv` only needs three
//! operations (ls-remote, clone --depth 1, rev-parse HEAD). The shell-out
//! is small, well-understood, and inherits the user's git config (auth,
//! credential helpers, proxy).

use crate::error::{AenvError, Result};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

static GIT_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Return whether `git --version` succeeds. Result is cached for the process.
pub fn git_available() -> bool {
    *GIT_AVAILABLE.get_or_init(|| {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Resolve a (url, ref_spec) pair to a commit SHA via `git ls-remote`.
/// When `ref_spec` is `None`, returns the SHA for HEAD.
pub fn git_resolve_ref(url: &str, ref_spec: Option<&str>) -> Result<String> {
    if !git_available() {
        return Err(AenvError::RemoteUnreachable(
            "git not on PATH".to_string(),
        ));
    }
    let mut cmd = Command::new("git");
    cmd.arg("ls-remote").arg(url);
    if let Some(r) = ref_spec {
        cmd.arg(r);
    }
    let output = cmd.output().map_err(|e| {
        AenvError::RemoteUnreachable(format!("git ls-remote {url}: {e}"))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AenvError::RemoteUnreachable(format!(
            "git ls-remote {url} failed: {}",
            stderr.trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // First field of the first non-empty line is the SHA.
    let sha = stdout
        .lines()
        .find_map(|line| line.split_whitespace().next())
        .ok_or_else(|| {
            AenvError::RemoteUnreachable(format!(
                "git ls-remote {url} returned no matching refs"
            ))
        })?;
    Ok(sha.to_string())
}

/// Shallow-clone `url` at `ref_spec` (or HEAD) into `dest`. Returns the
/// resolved commit SHA. `dest` must not exist (git will create it).
pub fn git_clone(url: &str, ref_spec: Option<&str>, dest: &Path) -> Result<String> {
    if !git_available() {
        return Err(AenvError::RemoteUnreachable(
            "git not on PATH".to_string(),
        ));
    }
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg("--depth").arg("1");
    if let Some(r) = ref_spec {
        cmd.arg("--branch").arg(r);
    }
    cmd.arg(url).arg(dest);
    let output = cmd.output().map_err(|e| {
        AenvError::RemoteUnreachable(format!("git clone {url}: {e}"))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AenvError::RemoteUnreachable(format!(
            "git clone {url} failed: {}",
            stderr.trim()
        )));
    }
    // Resolve the actual HEAD commit in the clone.
    let head = Command::new("git")
        .current_dir(dest)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| AenvError::RemoteUnreachable(format!("git rev-parse: {e}")))?;
    if !head.status.success() {
        let stderr = String::from_utf8_lossy(&head.stderr);
        return Err(AenvError::RemoteUnreachable(format!(
            "git rev-parse HEAD failed: {}",
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&head.stdout).trim().to_string())
}
```

Add to `crates/aenv-core/src/skills/mod.rs`:

```rust
pub mod git;
```

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_git_probe 2>&1 | tail -10`
Expected: PASS — 3 tests pass when git is on PATH; print "skipping" lines when not.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/skills/git.rs crates/aenv-core/src/skills/mod.rs crates/aenv-core/tests/skill_git_probe.rs
git commit -m "Add git wrapper (ls-remote, clone, availability probe)"
```

---

### Task 7: Git skill source resolver (with cache + Registry stub)

Wraps `git.rs` and `cache.rs` together. The flow:

1. Compute the cache key: `skill_cache_path(layout, source_string, ref_label)`.
2. If the cache dir exists, treat as resolved (don't re-clone). Look up resolved SHA via `git rev-parse HEAD` in the cache.
3. If not cached, call `git_clone(url, ref_spec, cache_dir)`.
4. Read `<cache_dir>/SKILL.md` for the content hash.

Also lands the `SourceKind::Registry` stub: returns `ManifestInvalid("not yet implemented...")`.

**Files:**
- Create: `crates/aenv-core/src/skills/git_source.rs`
- Create: `crates/aenv-core/src/skills/registry.rs`
- Modify: `crates/aenv-core/src/skills/mod.rs` (re-export both)
- Test: `crates/aenv-core/tests/skill_resolve_git.rs`
- Test: `crates/aenv-core/tests/skill_resolve_registry.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/aenv-core/tests/skill_resolve_git.rs`:

```rust
use aenv_core::fs::RealFilesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::skills::git::git_available;
use aenv_core::skills::git_source::resolve_git;
use std::process::Command;
use tempfile::tempdir;

fn skip_unless_git() -> bool {
    git_available()
}

fn make_bare_repo_with_skill() -> tempfile::TempDir {
    let bare = tempdir().unwrap();
    let bare_path = bare.path();
    Command::new("git").args(["init", "--bare"]).arg(bare_path).status().unwrap();
    let work = tempdir().unwrap();
    let work_path = work.path();
    Command::new("git").args(["clone"]).arg(bare_path).arg(work_path).status().unwrap();
    std::fs::create_dir_all(work_path.join("dummy-skill")).unwrap();
    std::fs::write(
        work_path.join("dummy-skill/SKILL.md"),
        b"---\nname: dummy-skill\ndescription: a test skill\n---\nbody\n",
    )
    .unwrap();
    Command::new("git").current_dir(work_path).args(["add", "."]).status().unwrap();
    Command::new("git")
        .current_dir(work_path)
        .args(["-c", "user.email=t@e", "-c", "user.name=t", "commit", "-m", "init"])
        .status()
        .unwrap();
    Command::new("git").current_dir(work_path).args(["push", "origin", "HEAD:master"]).status().unwrap();
    bare
}

#[test]
fn resolves_git_source_to_cache_directory() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_bare_repo_with_skill();
    let aenv_home = tempdir().unwrap();
    let layout = RegistryLayout::new(aenv_home.path().to_path_buf());
    let url = format!("file://{}", bare.path().display());

    let fs = RealFilesystem;
    let result = resolve_git(&fs, &layout, &url, Some("master"), "dummy-skill").unwrap();
    assert!(result.source_path.exists());
    assert!(result.source_path.join("dummy-skill/SKILL.md").exists());
    assert_eq!(result.resolved_ref.as_deref().map(|s| s.len()), Some(40));
    assert!(result.resolved_hash.starts_with("sha256:"));
}

#[test]
fn second_resolution_uses_cache() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_bare_repo_with_skill();
    let aenv_home = tempdir().unwrap();
    let layout = RegistryLayout::new(aenv_home.path().to_path_buf());
    let url = format!("file://{}", bare.path().display());
    let fs = RealFilesystem;

    let r1 = resolve_git(&fs, &layout, &url, Some("master"), "dummy-skill").unwrap();
    let r2 = resolve_git(&fs, &layout, &url, Some("master"), "dummy-skill").unwrap();
    assert_eq!(r1.source_path, r2.source_path);
    assert_eq!(r1.resolved_ref, r2.resolved_ref);
}

#[test]
fn unreachable_url_returns_remote_unreachable() {
    if !skip_unless_git() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let aenv_home = tempdir().unwrap();
    let layout = RegistryLayout::new(aenv_home.path().to_path_buf());
    let fs = RealFilesystem;
    let err =
        resolve_git(&fs, &layout, "file:///definitely/not/a/repo", None, "x").unwrap_err();
    assert_eq!(err.exit_code(), 14);
}
```

Create `crates/aenv-core/tests/skill_resolve_registry.rs`:

```rust
use aenv_core::skills::registry::resolve_registry;

#[test]
fn registry_returns_not_yet_implemented() {
    let err = resolve_registry("cite-evidence", None).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("registry") && msg.contains("not yet implemented"));
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_resolve_git --test skill_resolve_registry 2>&1 | tail -10`
Expected: FAIL — `git_source` and `registry` modules don't exist.

- [ ] **Step 3: Implement `git_source.rs`**

Create `crates/aenv-core/src/skills/git_source.rs`:

```rust
//! Resolve a git-URL skill source into a cached directory.
//!
//! Pinned sources cache under `<source-hash>/<ref>/`; unpinned sources
//! cache under `<source-hash>/head/`. A pre-existing cache directory is
//! reused (the `aenv skill refresh` command, deferred from Phase 4, will
//! invalidate it).

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::skills::cache::skill_cache_path;
use crate::skills::git::{git_clone, git_resolve_ref};
use crate::skills::local::LocalResolution;
use std::path::Path;

/// Result is `LocalResolution` because, once cloned, a git source behaves
/// like a local-path source for materialization purposes.
pub fn resolve_git<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    url: &str,
    ref_spec: Option<&str>,
    skill_name: &str,
) -> Result<LocalResolution> {
    let ref_label = ref_spec.unwrap_or("head").to_string();
    let cache_dir = skill_cache_path(layout, url, &ref_label);

    if fs.exists(&cache_dir)? {
        // Cached. Read the resolved SHA from the existing clone via shell-out.
        let resolved_sha = git_head_sha(&cache_dir)?;
        let local =
            crate::skills::local::resolve_local(fs, &cache_dir.join(skill_name), skill_name)
                .or_else(|_| {
                    // Some sources put SKILL.md at the root, not under <skill-name>/.
                    crate::skills::local::resolve_local(fs, &cache_dir, skill_name)
                })?;
        return Ok(LocalResolution {
            source_path: local.source_path,
            resolved_ref: Some(resolved_sha),
            resolved_hash: local.resolved_hash,
        });
    }

    // Not cached. Create parent, clone, then read.
    if let Some(parent) = cache_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AenvError::Io(std::io::Error::new(
                e.kind(),
                format!("create cache parent {}: {e}", parent.display()),
            ))
        })?;
    }
    let resolved_sha = git_clone(url, ref_spec, &cache_dir)?;
    let _ = git_resolve_ref; // intentionally unused: clone returns the SHA we need

    let local =
        crate::skills::local::resolve_local(fs, &cache_dir.join(skill_name), skill_name)
            .or_else(|_| crate::skills::local::resolve_local(fs, &cache_dir, skill_name))?;

    Ok(LocalResolution {
        source_path: local.source_path,
        resolved_ref: Some(resolved_sha),
        resolved_hash: local.resolved_hash,
    })
}

fn git_head_sha(dir: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| AenvError::RemoteUnreachable(format!("git rev-parse: {e}")))?;
    if !output.status.success() {
        return Err(AenvError::RemoteUnreachable(format!(
            "git rev-parse HEAD in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
```

- [ ] **Step 4: Implement `registry.rs`**

Create `crates/aenv-core/src/skills/registry.rs`:

```rust
//! Registry-source stub. Phase 4 does not resolve registry sources —
//! the registry design is still pending (PRD §3 open question).

use crate::error::{AenvError, Result};
use crate::skills::local::LocalResolution;

/// Always returns `ManifestInvalid` with a clear "not yet implemented" message.
pub fn resolve_registry(name: &str, _ref_spec: Option<&str>) -> Result<LocalResolution> {
    Err(AenvError::ManifestInvalid(format!(
        "registry source 'registry:{name}' is not yet implemented \
         (pending PRD §3 registry design)"
    )))
}
```

Add to `crates/aenv-core/src/skills/mod.rs`:

```rust
pub mod git_source;
pub mod registry;
```

- [ ] **Step 5: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_resolve_git --test skill_resolve_registry 2>&1 | tail -15`
Expected: PASS — registry test always passes; git tests pass when git is on PATH (3 + 1 = 4 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/skills/git_source.rs crates/aenv-core/src/skills/registry.rs crates/aenv-core/src/skills/mod.rs crates/aenv-core/tests/skill_resolve_git.rs crates/aenv-core/tests/skill_resolve_registry.rs
git commit -m "Add git skill source resolver + Registry stub"
```

---

### Task 8: Unified skill resolver + `required = true` semantics

A single entry point `resolve_imported_skill(fs, layout, decl) -> Result<LocalResolution>` that dispatches by `SourceKind` (Local / Git / Registry). On error, the caller decides whether to abort or warn-and-skip based on `decl.required`.

This task creates the unified resolver and the per-decl helper that applies the `required` rule.

**Files:**
- Modify: `crates/aenv-core/src/skills/mod.rs` — add `resolve_imported_skill`, add `apply_required_rule`
- Test: `crates/aenv-core/tests/skill_required.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/skill_required.rs`:

```rust
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::skills::{apply_required_rule, resolve_imported_skill, SkillDecl, SkillMode};
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv-home"))
}

#[test]
fn resolves_when_local_source_exists() {
    let fs = MockFilesystem::new();
    fs.write(
        &PathBuf::from("/local/skill/SKILL.md"),
        b"---\nname: x\ndescription: y\n---\n",
    )
    .unwrap();
    let decl = SkillDecl {
        name: "my-skill".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/local/skill".into()),
        ref_: None,
        required: false,
    };
    let result = resolve_imported_skill(&fs, &layout(), &decl).unwrap();
    assert_eq!(result.source_path, PathBuf::from("/local/skill"));
}

#[test]
fn required_unreachable_propagates_error() {
    let fs = MockFilesystem::new();
    let decl = SkillDecl {
        name: "missing".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/does/not/exist".into()),
        ref_: None,
        required: true,
    };
    let outcome = apply_required_rule(&fs, &layout(), &decl);
    let err = outcome.expect_err("required + missing should error");
    // The conversion to ActivationConflict happens at the activation layer
    // (Task 9). At this layer the underlying error propagates.
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn unrequired_unreachable_returns_skipped_marker() {
    let fs = MockFilesystem::new();
    let decl = SkillDecl {
        name: "optional".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/does/not/exist".into()),
        ref_: None,
        required: false,
    };
    // apply_required_rule returns Ok(None) when the skill is not required
    // and resolution failed.
    let outcome = apply_required_rule(&fs, &layout(), &decl).unwrap();
    assert!(outcome.is_none());
}

#[test]
fn authored_decls_panic_or_error() {
    // apply_required_rule is for imported skills. Authored skills should
    // be filtered out before they reach this function. We surface a clear
    // error to help diagnose misuse.
    let fs = MockFilesystem::new();
    let decl = SkillDecl {
        name: "x".into(),
        mode: SkillMode::Authored,
        adapter: None,
        source: None,
        ref_: None,
        required: false,
    };
    let err = apply_required_rule(&fs, &layout(), &decl).unwrap_err();
    assert!(err.to_string().contains("authored"));
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_required 2>&1 | tail -10`
Expected: FAIL — `resolve_imported_skill` and `apply_required_rule` don't exist.

- [ ] **Step 3: Implement the resolver**

Append to `crates/aenv-core/src/skills/mod.rs`:

```rust
use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::skills::local::LocalResolution;
use crate::skills::source::SourceKind;

/// Resolve an imported skill decl into a `LocalResolution`.
///
/// Dispatches by `SourceKind`. Errors propagate; the caller decides whether
/// to apply the `required = true` rule (see `apply_required_rule`).
pub fn resolve_imported_skill<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    decl: &SkillDecl,
) -> Result<LocalResolution> {
    if !matches!(decl.mode, SkillMode::Imported) {
        return Err(AenvError::ManifestInvalid(format!(
            "skill '{}' is authored — use authored-skill resolution instead",
            decl.name
        )));
    }
    let source_str = decl.source.as_deref().ok_or_else(|| {
        AenvError::ManifestInvalid(format!(
            "imported skill '{}' has no source",
            decl.name
        ))
    })?;
    let kind = SourceKind::parse(source_str)?;
    match kind {
        SourceKind::Local(path) => crate::skills::local::resolve_local(fs, &path, &decl.name),
        SourceKind::Git { url, ref_spec } => {
            // Use the decl's ref if provided; else use the URL fragment ref.
            let chosen = decl.ref_.as_deref().or(ref_spec.as_deref());
            crate::skills::git_source::resolve_git(fs, layout, &url, chosen, &decl.name)
        }
        SourceKind::Registry(name) => {
            crate::skills::registry::resolve_registry(&name, decl.ref_.as_deref())
        }
    }
}

/// Resolve, then apply the `required = true` rule.
///
/// Returns:
/// * `Ok(Some(resolution))` when resolution succeeded.
/// * `Ok(None)` when resolution failed AND the skill is not required —
///   caller should emit a warning and continue.
/// * `Err(_)` when resolution failed AND the skill is required.
pub fn apply_required_rule<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    decl: &SkillDecl,
) -> Result<Option<LocalResolution>> {
    match resolve_imported_skill(fs, layout, decl) {
        Ok(r) => Ok(Some(r)),
        Err(e) => {
            if decl.required {
                Err(e)
            } else {
                Ok(None)
            }
        }
    }
}
```

- [ ] **Step 4: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test skill_required 2>&1 | tail -10`
Expected: PASS — 4 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/skills/mod.rs crates/aenv-core/tests/skill_required.rs
git commit -m "Add unified skill resolver + required=true semantics"
```

---

### Task 9: Wire skill candidates into `resolve_namespace` + state schema 4

This is the integration task. After Phase 2's candidate-gathering walk, we add a pass that:

1. For each namespace in the chain, look at its `[[skills]]`.
2. For each authored skill, the adapter-determined path is `<adapter.skills_dir>/<skill.name>/` (claude-code's `skills_dir` is `.claude/skills`). Walk the namespace's own directory at that path and emit Candidates for every file under it. Adapter is the skill's declared adapter.
3. For each imported skill, call `apply_required_rule`. If `Ok(Some(resolution))`, walk the resolution's `source_path` and emit Candidates pointing at the cached files. If `Ok(None)`, emit a warning to stderr (`eprintln!`) but no candidates. If `Err`, return early (the activation will fail).
4. Imported-skill candidates carry their `SkillProvenance` in a new field (`skill_provenance: Option<SkillProvenance>`) for the state file.

Also: `Adapter` gains an optional `skills_dir: Option<String>` field (default for claude-code: `.claude/skills`). Other adapters can declare their own.

Schema 4 bumps `ActivationState` to record `skill_provenance: Option<SkillProvenance>` per `ManagedFile`.

**Files:**
- Modify: `crates/aenv-core/src/adapter.rs` — add `skills_dir: Option<String>`; update each `claude-code` adapter TOML to declare `skills_dir = ".claude/skills"`.
- Modify: `crates/aenv-core/src/resolve.rs` — `Candidate` gains `skill_provenance: Option<SkillProvenance>`; `gather_candidates` adds skill-walking pass.
- Modify: `crates/aenv-core/src/state.rs` — bump `SCHEMA_VERSION` to 4; `ManagedFile` gains optional `skill_provenance` field; define `SkillProvenance` struct.
- Modify: `crates/aenv-core/src/activate/mod.rs` — populate `skill_provenance` on materialization.
- Modify: `crates/aenv-core/src/adapters_builtin/claude_code.toml` — add `skills_dir = ".claude/skills"`.
- Test: `crates/aenv-core/tests/activate_authored_skill.rs`
- Test: `crates/aenv-core/tests/activate_imported_skill_local.rs`
- Test: `crates/aenv-core/tests/state_schema_4.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/aenv-core/tests/activate_authored_skill.rs`:

```rust
use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::PathBuf;

fn write(fs: &MockFilesystem, p: &str, b: &[u8]) {
    fs.write(&PathBuf::from(p), b).unwrap();
}

#[test]
fn authored_skill_materializes_at_project_path() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    // Adapter with skills_dir set.
    write(
        &fs,
        "/h/adapters/claude-code.toml",
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\nskills_dir = \".claude/skills\"\n",
    );

    // Namespace with one authored skill.
    write(
        &fs,
        "/h/envs/base/aenv.toml",
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[[skills]]\nname = \"my-skill\"\nmode = \"authored\"\nadapter = \"claude-code\"\n",
    );
    write(&fs, "/h/envs/base/CLAUDE.md", b"hi");
    write(
        &fs,
        "/h/envs/base/.claude/skills/my-skill/SKILL.md",
        b"---\nname: my-skill\ndescription: y\n---\nbody\n",
    );

    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("base").unwrap(),
    )
    .unwrap();
    let paths: Vec<_> = state.managed_files.iter().map(|m| m.path.clone()).collect();
    assert!(paths.contains(&PathBuf::from(".claude/skills/my-skill/SKILL.md")));
    assert!(fs.exists(&project.join(".claude/skills/my-skill/SKILL.md")).unwrap());
}
```

Create `crates/aenv-core/tests/activate_imported_skill_local.rs`:

```rust
use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::PathBuf;

fn write(fs: &MockFilesystem, p: &str, b: &[u8]) {
    fs.write(&PathBuf::from(p), b).unwrap();
}

#[test]
fn imported_local_skill_materializes_from_source() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    write(
        &fs,
        "/h/adapters/claude-code.toml",
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\nskills_dir = \".claude/skills\"\n",
    );

    // External skill source
    write(
        &fs,
        "/external/my-import/SKILL.md",
        b"---\nname: my-import\ndescription: yo\n---\nbody\n",
    );

    // Namespace declares an imported skill from a local path.
    write(
        &fs,
        "/h/envs/base/aenv.toml",
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[[skills]]\nname = \"my-import\"\nmode = \"imported\"\nadapter = \"claude-code\"\nsource = \"/external/my-import\"\n",
    );
    write(&fs, "/h/envs/base/CLAUDE.md", b"hi");

    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("base").unwrap(),
    )
    .unwrap();
    let imported = state
        .managed_files
        .iter()
        .find(|m| m.path == PathBuf::from(".claude/skills/my-import/SKILL.md"))
        .expect("imported skill should appear in managed files");
    assert!(
        imported.skill_provenance.is_some(),
        "expected skill_provenance on imported file"
    );
    let prov = imported.skill_provenance.as_ref().unwrap();
    assert_eq!(prov.source, "/external/my-import");
    assert!(prov.resolved_hash.starts_with("sha256:"));
}
```

Create `crates/aenv-core/tests/state_schema_4.rs`:

```rust
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::resolve::MaterializeStrategy;
use aenv_core::state::{ActivationState, ManagedFile, SkillProvenance, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn schema_version_is_4() {
    assert_eq!(SCHEMA_VERSION, 4);
}

#[test]
fn schema_4_roundtrips_with_skill_provenance() {
    let qn = QualifiedName::new(
        NamespaceId::new("base").unwrap(),
        ShortName::new(".claude/skills/x/SKILL.md").unwrap(),
    );
    let state = ActivationState {
        schema_version: 4,
        active_namespace: "base".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![ManagedFile {
            path: PathBuf::from(".claude/skills/x/SKILL.md"),
            qualified_name: qn,
            strategy: MaterializeStrategy::Symlink,
            contributors: vec![],
            shadows: vec![],
            skill_provenance: Some(SkillProvenance {
                source: "/external/x".into(),
                resolved_ref: None,
                resolved_hash: "sha256:abc".into(),
            }),
        }],
        backed_up: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
    };
    let s = state.to_json().unwrap();
    let parsed = ActivationState::from_json(&s).unwrap();
    assert_eq!(parsed, state);
}

#[test]
fn reads_schema_3_with_no_skill_provenance() {
    let json = r#"{
        "schema_version": 3,
        "active_namespace": "base",
        "project_root": "/p",
        "managed_files": [{
            "path": "CLAUDE.md",
            "qualified_name": "base::CLAUDE.md",
            "strategy": "symlink",
            "contributors": [],
            "shadows": []
        }],
        "backed_up": [],
        "parameters": {},
        "policies": {}
    }"#;
    let parsed = ActivationState::from_json(json).unwrap();
    assert_eq!(parsed.schema_version, 3);
    assert!(parsed.managed_files[0].skill_provenance.is_none());
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test activate_authored_skill --test activate_imported_skill_local --test state_schema_4 2>&1 | tail -10`
Expected: FAIL — `SkillProvenance` and `Adapter::skills_dir` don't exist; schema 4 not bumped.

- [ ] **Step 3: Extend `state.rs`**

In `crates/aenv-core/src/state.rs`:
- Bump `SCHEMA_VERSION` to `4`.
- Add a new `SkillProvenance` struct: `{ source: String, resolved_ref: Option<String>, resolved_hash: String }` with `Serialize, Deserialize, Debug, Clone, PartialEq, Eq`.
- Add `skill_provenance: Option<SkillProvenance>` field to `ManagedFile` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- The custom `Deserialize` on `ManagedFile` and `ActivationState` already uses `#[serde(default)]` per-field; just add the new field to the `Raw` struct (default = `None`).

- [ ] **Step 4: Extend `Adapter`**

In `crates/aenv-core/src/adapter.rs`, add to the `Adapter` struct:

```rust
/// Adapter-specific directory under which skills are materialized in the
/// project. Defaults to `None` (the adapter has no skill convention).
/// For claude-code this is `.claude/skills`.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub skills_dir: Option<String>,
```

Update `crates/aenv-core/src/adapters_builtin/claude_code.toml` to add the field:

```toml
name = "claude-code"
files = ["CLAUDE.md", ".claude/"]
skills_dir = ".claude/skills"

[roles]
"CLAUDE.md" = "instructions"
```

- [ ] **Step 5: Extend `Candidate` and `resolve_namespace`**

In `crates/aenv-core/src/resolve.rs`:

1. Add `pub skill_provenance: Option<SkillProvenance>` field to `Candidate`. Default to `None` for non-skill candidates. Import `SkillProvenance` from `crate::state`.

2. After the existing `gather_candidates` call for each namespace's adapter files, also call a new `gather_skill_candidates(fs, layout, ns, manifest, adapters, &mut candidates)` function. The function:
   - For each `SkillDecl` in `manifest.skills`:
     - Determine the adapter name (decl.adapter, or the only entry in `manifest.adapters` if exactly one, else `Err(ManifestInvalid)`).
     - Look up the adapter; if `skills_dir` is `None`, return `Err(ManifestInvalid("adapter has no skills_dir"))`.
     - Compute the destination path: `<adapter.skills_dir>/<skill.name>/`.
     - For `Authored`: walk `<ns_root>/<dest>/` and emit one Candidate per file with `source_path` under the namespace directory.
     - For `Imported`: call `apply_required_rule(fs, layout, decl)`. On `Some(resolution)`, walk `resolution.source_path` and emit Candidates with `source_path` under the cache directory and `skill_provenance = Some(SkillProvenance::from(&resolution, source_str))`. On `None`, `eprintln!("[aenv] skill '{}' from '{}' unreachable; skipping", decl.name, source)` and emit nothing. On `Err`, propagate.

3. Update every `Candidate { ... }` struct literal in tests to add `skill_provenance: None`. Find with: `grep -rn "Candidate {" crates/`.

- [ ] **Step 6: Materialize the new field in state**

In `crates/aenv-core/src/activate/mod.rs`, where `ManagedFile` is constructed, propagate the `skill_provenance` from the `Candidate`. The propagation is mechanical — pass through.

- [ ] **Step 7: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test activate_authored_skill --test activate_imported_skill_local --test state_schema_4 2>&1 | tail -15`
Expected: PASS — 4 tests.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | tail -5`
Expected: workspace green.

- [ ] **Step 8: Commit**

```bash
git add -u
git add crates/aenv-core/tests/activate_authored_skill.rs crates/aenv-core/tests/activate_imported_skill_local.rs crates/aenv-core/tests/state_schema_4.rs
git commit -m "Wire skill candidates through resolution + state schema 4"
```

---

### Task 10: Make `required = true` failure into `ActivationConflict` (exit 13)

When `apply_required_rule` returns `Err`, the activation layer must wrap that error as `ActivationConflict` per PRD R-22 — and abort before any file is materialized (R-63: project must remain untouched).

This was already partially done in Task 9 (errors propagate). This task adds the wrapping and the integration test.

**Files:**
- Modify: `crates/aenv-core/src/resolve.rs` — wrap `apply_required_rule` errors as `ResolutionError::ManifestInvalid` (which maps to `ActivationConflict` is wrong — we want exit 13 ActivationConflict specifically). Actually wrap to a new conversion path: see step 3.
- Test: `crates/aenv-core/tests/activate_required_skill_missing.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/activate_required_skill_missing.rs`:

```rust
use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::AenvError;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::PathBuf;

fn write(fs: &MockFilesystem, p: &str, b: &[u8]) {
    fs.write(&PathBuf::from(p), b).unwrap();
}

#[test]
fn required_unreachable_aborts_activation() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    write(
        &fs,
        "/h/adapters/claude-code.toml",
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\nskills_dir = \".claude/skills\"\n",
    );
    write(
        &fs,
        "/h/envs/base/aenv.toml",
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[[skills]]\nname = \"missing\"\nmode = \"imported\"\nadapter = \"claude-code\"\nsource = \"/does/not/exist\"\nrequired = true\n",
    );
    write(&fs, "/h/envs/base/CLAUDE.md", b"hi");

    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let err = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("base").unwrap(),
    )
    .unwrap_err();
    assert!(
        matches!(err, AenvError::ActivationConflict(_)),
        "expected ActivationConflict, got {err:?}"
    );
    assert_eq!(err.exit_code(), 13);
    // Project must be untouched (R-63).
    assert!(!fs.exists(&project.join(".aenv-state/state.json")).unwrap());
    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn unrequired_unreachable_warns_but_activates() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    write(
        &fs,
        "/h/adapters/claude-code.toml",
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\nskills_dir = \".claude/skills\"\n",
    );
    write(
        &fs,
        "/h/envs/base/aenv.toml",
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[[skills]]\nname = \"optional\"\nmode = \"imported\"\nadapter = \"claude-code\"\nsource = \"/does/not/exist\"\n",
    );
    write(&fs, "/h/envs/base/CLAUDE.md", b"hi");

    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("base").unwrap(),
    )
    .expect("activation should succeed when optional skill is unreachable");
    assert_eq!(state.active_namespace, "base");
    // CLAUDE.md materialized; skill is absent.
    assert!(fs.exists(&project.join("CLAUDE.md")).unwrap());
    assert!(!fs.exists(&project.join(".claude/skills/optional/SKILL.md")).unwrap());
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test activate_required_skill_missing 2>&1 | tail -10`
Expected: FAIL — the error currently propagates as `ResolutionError::ManifestInvalid` (exit 12), not `ActivationConflict` (exit 13).

- [ ] **Step 3: Add the conversion**

In `crates/aenv-core/src/resolve.rs`, wherever `apply_required_rule` is called from `gather_skill_candidates`:

```rust
match crate::skills::apply_required_rule(fs, layout, decl) {
    Ok(Some(resolution)) => { /* emit candidates */ }
    Ok(None) => {
        eprintln!(
            "[aenv] skill '{}' from '{}' unreachable; skipping (not required)",
            decl.name,
            decl.source.as_deref().unwrap_or("<no source>")
        );
    }
    Err(e) => {
        // Required+unreachable. Convert to ActivationConflict for exit 13
        // per PRD R-22.
        return Err(ResolutionError::ActivationConflict(format!(
            "required skill '{}' unreachable: {}",
            decl.name, e
        )));
    }
}
```

Add the `ActivationConflict` variant to `ResolutionError` and extend the `From<ResolutionError> for AenvError` impl to map it to `AenvError::ActivationConflict(msg)`.

- [ ] **Step 4: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test activate_required_skill_missing 2>&1 | tail -10`
Expected: PASS — 2 tests passed.

- [ ] **Step 5: Commit**

```bash
git add -u
git add crates/aenv-core/tests/activate_required_skill_missing.rs
git commit -m "Map required-skill failures to ActivationConflict (exit 13)"
```

---

### Task 11: `aenv status` shows skills section

After the existing managed-files block, group by skill provenance and print:
- `Skills (N authored, M imported):` header
- One line per skill with its qualified name + mode + source + resolved ref

A skill is "this file's `skill_provenance` is `Some(...)`" OR "this file's path starts with the adapter's `skills_dir` for any active adapter" (for authored skills, which have no provenance recorded — they're indistinguishable from regular adapter files unless we filter by path).

The cleanest approach: the resolver attaches `skill_provenance: Some(SkillProvenance { source: "<authored:namespace>", resolved_ref: None, resolved_hash: <content-hash> })` to authored-skill candidates too. Then `aenv status` can group purely on `skill_provenance.is_some()` and a synthetic source prefix discriminates authored vs imported.

This needs a small change to Task 9's `gather_skill_candidates`: emit `skill_provenance` for authored skills too, with source `format!("authored:{}", ns.as_str())`.

**Files:**
- Modify: `crates/aenv-core/src/resolve.rs` — authored skills also get `skill_provenance`
- Modify: `crates/aenv-cli/src/cmd/status.rs` — add Skills section
- Test: `crates/aenv-cli/tests/status_skills_section.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-cli/tests/status_skills_section.rs`:

```rust
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: std::path::PathBuf,
    project: std::path::PathBuf,
}

impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
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
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
        c.env("AENV_HOME", &self.aenv_home);
        c
    }

    fn aenv_home(&self) -> &Path {
        &self.aenv_home
    }

    fn project(&self) -> &Path {
        &self.project
    }
}

#[test]
fn status_lists_authored_skill() {
    let h = Harness::new();
    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "my-skill"
mode = "authored"
adapter = "claude-code"
"#,
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/base/CLAUDE.md"), "hi").unwrap();
    std::fs::create_dir_all(h.aenv_home().join("envs/base/.claude/skills/my-skill")).unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/.claude/skills/my-skill/SKILL.md"),
        "---\nname: my-skill\ndescription: y\n---\nbody\n",
    )
    .unwrap();

    h.cmd().args(["use", "base"]).current_dir(h.project()).output().unwrap();
    h.cmd().args(["activate"]).current_dir(h.project()).output().unwrap();

    let out = h.cmd().args(["status"]).current_dir(h.project()).output().unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Skills"), "stdout missing Skills section: {stdout}");
    assert!(stdout.contains("my-skill"), "stdout missing skill name: {stdout}");
    assert!(stdout.contains("authored"), "stdout missing 'authored' mode: {stdout}");
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test status_skills_section 2>&1 | tail -10`
Expected: FAIL — status doesn't print a Skills section.

- [ ] **Step 3: Attach `skill_provenance` to authored-skill candidates**

In `crates/aenv-core/src/resolve.rs`, within the authored-skill branch of `gather_skill_candidates`, populate `skill_provenance: Some(SkillProvenance { source: format!("authored:{}", ns), resolved_ref: None, resolved_hash: format!("sha256:{}", sha256_of_file_contents) })`. For the SKILL.md the hash is meaningful; for other files under the skill dir, hash each file's contents.

(Pragmatic alternative: only attach `skill_provenance` to the SKILL.md, not every file under the skill directory. That keeps state.json small and matches the "one skill = one provenance entry" mental model. Pick this approach.)

The SKILL.md file is `<skill-dir>/<skill-name>/SKILL.md`. Detect by suffix.

- [ ] **Step 4: Print Skills section in status.rs**

In `crates/aenv-cli/src/cmd/status.rs`, after the managed-files / backed-up / Parameters / Active policies sections, add:

```rust
// Skills section: group managed files by skill_provenance.
let skill_files: Vec<&ManagedFile> = state
    .managed_files
    .iter()
    .filter(|m| m.skill_provenance.is_some() && m.path.file_name() == Some("SKILL.md".as_ref()))
    .collect();
if !skill_files.is_empty() {
    let authored_count = skill_files
        .iter()
        .filter(|m| {
            m.skill_provenance
                .as_ref()
                .map(|p| p.source.starts_with("authored:"))
                .unwrap_or(false)
        })
        .count();
    let imported_count = skill_files.len() - authored_count;
    out.push('\n');
    out.push_str(&format!(
        "Skills ({authored_count} authored, {imported_count} imported):\n"
    ));
    for m in &skill_files {
        let prov = m.skill_provenance.as_ref().unwrap();
        let (mode, source) = if prov.source.starts_with("authored:") {
            ("authored", "-".to_string())
        } else {
            ("imported", prov.source.clone())
        };
        let ref_part = prov
            .resolved_ref
            .as_ref()
            .map(|r| format!(" @ {r}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "  {}  {mode}  {source}{ref_part}\n",
            m.qualified_name
        ));
    }
}
```

- [ ] **Step 5: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test status_skills_section 2>&1 | tail -10`
Expected: PASS — 1 test passed.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | tail -5`
Expected: workspace green.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/resolve.rs crates/aenv-cli/src/cmd/status.rs crates/aenv-cli/tests/status_skills_section.rs
git commit -m "Show Skills section in 'aenv status'"
```

---

### Task 12: `aenv skill new` command

`aenv skill new <name> --ns <ns> [--adapter <a>]` scaffolds an authored skill: creates `<ns-dir>/<adapter.skills_dir>/<name>/SKILL.md` with minimal frontmatter (`name`, `description: "TODO: describe this skill"`), then appends a `[[skills]]` entry to the namespace manifest.

If `--adapter` is omitted and the namespace declares exactly one adapter, use that. Otherwise error.

**Files:**
- Create: `crates/aenv-cli/src/cmd/skill/mod.rs`
- Create: `crates/aenv-cli/src/cmd/skill/new.rs`
- Modify: `crates/aenv-cli/src/cmd/mod.rs` — `pub mod skill;`
- Modify: `crates/aenv-cli/src/main.rs` — add `Skill { action }` subcommand
- Test: `crates/aenv-cli/tests/skill_new_e2e.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-cli/tests/skill_new_e2e.rs`:

```rust
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: std::path::PathBuf,
    project: std::path::PathBuf,
}

impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
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
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
        c.env("AENV_HOME", &self.aenv_home);
        c
    }
    fn aenv_home(&self) -> &Path {
        &self.aenv_home
    }
    fn project(&self) -> &Path {
        &self.project
    }
}

#[test]
fn skill_new_scaffolds_skill_md_and_appends_manifest() {
    let h = Harness::new();
    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        "name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/base/CLAUDE.md"), "hi").unwrap();

    let out = h
        .cmd()
        .args(["skill", "new", "my-skill", "--ns", "base"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let skill_md = h.aenv_home().join("envs/base/.claude/skills/my-skill/SKILL.md");
    assert!(skill_md.exists(), "SKILL.md not created");
    let body = std::fs::read_to_string(&skill_md).unwrap();
    assert!(body.contains("name: my-skill"));
    assert!(body.contains("description:"));

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(manifest.contains("[[skills]]"));
    assert!(manifest.contains("name = \"my-skill\""));
    assert!(manifest.contains("mode = \"authored\""));
}

#[test]
fn skill_new_errors_when_namespace_missing() {
    let h = Harness::new();
    let out = h
        .cmd()
        .args(["skill", "new", "x", "--ns", "ghost"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(10));
}

#[test]
fn skill_new_errors_when_adapter_ambiguous() {
    let h = Harness::new();
    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        "name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[adapters.cursor]\nfiles = [\".cursorrules\"]\n",
    )
    .unwrap();
    let out = h
        .cmd()
        .args(["skill", "new", "x", "--ns", "base"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("adapter"), "stderr = {stderr}");
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test skill_new_e2e 2>&1 | tail -10`
Expected: FAIL — `skill` subcommand doesn't exist.

- [ ] **Step 3: Implement the command**

Create `crates/aenv-cli/src/cmd/skill/mod.rs`:

```rust
//! `aenv skill <action>` subcommands.

pub mod new;
```

Create `crates/aenv-cli/src/cmd/skill/new.rs`:

```rust
//! `aenv skill new <name> --ns <ns> [--adapter <a>]` — scaffold authored skill.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::{SkillDecl, SkillMode};

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    namespace: &str,
    skill_name: &str,
    adapter_arg: Option<&str>,
) -> Result<()> {
    let manifest_path = layout.manifest_path(namespace);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(namespace.to_string()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        AenvError::ManifestInvalid(format!("manifest not utf-8: {e}"))
    })?;
    let mut manifest = AenvManifest::from_toml(text)?;

    // Choose adapter.
    let adapter_name = match adapter_arg {
        Some(a) => a.to_string(),
        None => {
            if manifest.adapters.len() != 1 {
                return Err(AenvError::ManifestInvalid(format!(
                    "namespace '{namespace}' declares {} adapters; use --adapter to disambiguate",
                    manifest.adapters.len()
                )));
            }
            manifest.adapters.keys().next().unwrap().clone()
        }
    };

    let adapter = adapters.get(&adapter_name).ok_or_else(|| {
        AenvError::AdapterMissing(adapter_name.clone())
    })?;
    let skills_dir = adapter.skills_dir.as_deref().ok_or_else(|| {
        AenvError::ManifestInvalid(format!(
            "adapter '{adapter_name}' has no skills_dir; cannot scaffold skills"
        ))
    })?;

    // Reject duplicate name.
    if manifest.skills.iter().any(|s| s.name == skill_name) {
        return Err(AenvError::ManifestInvalid(format!(
            "namespace '{namespace}' already declares a skill '{skill_name}'"
        )));
    }

    // Scaffold SKILL.md.
    let skill_md_path = layout
        .namespace_dir(namespace)
        .join(skills_dir)
        .join(skill_name)
        .join("SKILL.md");
    let body = format!(
        "---\nname: {skill_name}\ndescription: TODO: describe this skill\n---\n\n# {skill_name}\n\nDescribe when the agent should invoke this skill.\n"
    );
    fs.write(&skill_md_path, body.as_bytes())?;

    // Append [[skills]] to the manifest.
    manifest.skills.push(SkillDecl {
        name: skill_name.to_string(),
        mode: SkillMode::Authored,
        adapter: Some(adapter_name),
        source: None,
        ref_: None,
        required: false,
    });
    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    println!("Created authored skill '{skill_name}' in namespace '{namespace}'");
    Ok(())
}
```

Add to `crates/aenv-cli/src/cmd/mod.rs`:

```rust
pub mod skill;
```

In `crates/aenv-cli/src/main.rs`, add the clap variant:

```rust
/// Skill operations.
Skill {
    #[command(subcommand)]
    action: SkillAction,
},
```

And the action subcommand enum (alongside `AdapterAction`):

```rust
#[derive(Debug, Subcommand)]
enum SkillAction {
    /// Scaffold a new authored skill in a namespace.
    New {
        /// Skill name (becomes the directory name).
        name: String,
        /// Target namespace.
        #[arg(long)]
        ns: String,
        /// Adapter (defaults to the namespace's only adapter if exactly one).
        #[arg(long)]
        adapter: Option<String>,
    },
}
```

And the dispatch arm:

```rust
Command::Skill { action } => match action {
    SkillAction::New { name, ns, adapter } => {
        let adapters_reg =
            aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir())?;
        cmd::skill::new::run(&fs, &layout, &adapters_reg, &ns, &name, adapter.as_deref())
    }
},
```

- [ ] **Step 4: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test skill_new_e2e 2>&1 | tail -10`
Expected: PASS — 3 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-cli/src/cmd/skill/ crates/aenv-cli/src/cmd/mod.rs crates/aenv-cli/src/main.rs crates/aenv-cli/tests/skill_new_e2e.rs
git commit -m "Add 'aenv skill new' command"
```

---

### Task 13: `aenv skill import` command

`aenv skill import <source> --ns <ns> [--adapter <a>] [--pin <ref>]` adds an imported-skill entry to a namespace manifest. When `--pin` is given, also resolves the source (to verify reachability + write the pinned ref).

The `<source>` is the same form `SourceKind::parse` accepts.

**Files:**
- Create: `crates/aenv-cli/src/cmd/skill/import.rs`
- Modify: `crates/aenv-cli/src/cmd/skill/mod.rs` (add `pub mod import;`)
- Modify: `crates/aenv-cli/src/main.rs` — add `Import` variant to `SkillAction`
- Test: `crates/aenv-cli/tests/skill_import_local_e2e.rs`
- Test: `crates/aenv-cli/tests/skill_import_git_e2e.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/aenv-cli/tests/skill_import_local_e2e.rs`:

```rust
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: std::path::PathBuf,
    project: std::path::PathBuf,
}
impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        let project = std::fs::canonicalize(project_guard.path()).unwrap();
        Self { _aenv_home_guard: aenv_home_guard, _project_guard: project_guard, aenv_home, project }
    }
    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
        c.env("AENV_HOME", &self.aenv_home);
        c
    }
    fn aenv_home(&self) -> &Path { &self.aenv_home }
    fn project(&self) -> &Path { &self.project }
}

#[test]
fn import_local_path_adds_manifest_entry() {
    let h = Harness::new();
    let src = tempdir().unwrap();
    std::fs::write(
        src.path().join("SKILL.md"),
        "---\nname: external\ndescription: x\n---\n",
    )
    .unwrap();

    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        "name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let canonical_source = std::fs::canonicalize(src.path()).unwrap();
    let out = h
        .cmd()
        .args(["skill", "import"])
        .arg(canonical_source.to_str().unwrap())
        .args(["--ns", "base", "--adapter", "claude-code"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(manifest.contains("[[skills]]"));
    assert!(manifest.contains("mode = \"imported\""));
    assert!(manifest.contains(canonical_source.to_str().unwrap()));
}
```

Create `crates/aenv-cli/tests/skill_import_git_e2e.rs`:

```rust
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn git_available() -> bool {
    Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: std::path::PathBuf,
    project: std::path::PathBuf,
}
impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        let project = std::fs::canonicalize(project_guard.path()).unwrap();
        Self { _aenv_home_guard: aenv_home_guard, _project_guard: project_guard, aenv_home, project }
    }
    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
        c.env("AENV_HOME", &self.aenv_home);
        c
    }
    fn aenv_home(&self) -> &Path { &self.aenv_home }
    fn project(&self) -> &Path { &self.project }
}

fn make_repo_with_skill() -> tempfile::TempDir {
    let bare = tempdir().unwrap();
    Command::new("git").args(["init", "--bare"]).arg(bare.path()).status().unwrap();
    let work = tempdir().unwrap();
    Command::new("git").args(["clone"]).arg(bare.path()).arg(work.path()).status().unwrap();
    std::fs::create_dir_all(work.path().join("my-skill")).unwrap();
    std::fs::write(
        work.path().join("my-skill/SKILL.md"),
        "---\nname: my-skill\ndescription: y\n---\n",
    )
    .unwrap();
    Command::new("git").current_dir(work.path()).args(["add", "."]).status().unwrap();
    Command::new("git")
        .current_dir(work.path())
        .args(["-c", "user.email=t@e", "-c", "user.name=t", "commit", "-m", "init"])
        .status()
        .unwrap();
    Command::new("git").current_dir(work.path()).args(["push", "origin", "HEAD:master"]).status().unwrap();
    bare
}

#[test]
fn import_git_pinned_writes_resolved_ref() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }
    let bare = make_repo_with_skill();
    let h = Harness::new();
    h.cmd().args(["create", "base"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/base/aenv.toml"),
        "name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let url = format!("git+file://{}", bare.path().display());
    let out = h
        .cmd()
        .args(["skill", "import"])
        .arg(&url)
        .args(["--ns", "base", "--adapter", "claude-code", "--pin", "master"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let manifest = std::fs::read_to_string(h.aenv_home().join("envs/base/aenv.toml")).unwrap();
    // Some git SHA was written as the pinned ref.
    assert!(manifest.contains("ref ="));
    // It should be a 40-char hex string (full SHA) or the branch name as a fallback.
    assert!(manifest.contains("master") || manifest.contains("ref = \""));
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test skill_import_local_e2e --test skill_import_git_e2e 2>&1 | tail -10`
Expected: FAIL — `skill import` subcommand doesn't exist.

- [ ] **Step 3: Implement the command**

Create `crates/aenv-cli/src/cmd/skill/import.rs`:

```rust
//! `aenv skill import <source> --ns <ns> [--adapter <a>] [--pin <ref>]`

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::{apply_required_rule, SkillDecl, SkillMode};

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    namespace: &str,
    source: &str,
    adapter_arg: Option<&str>,
    pin: Option<&str>,
) -> Result<()> {
    let manifest_path = layout.manifest_path(namespace);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(namespace.to_string()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        AenvError::ManifestInvalid(format!("manifest not utf-8: {e}"))
    })?;
    let mut manifest = AenvManifest::from_toml(text)?;

    let adapter_name = match adapter_arg {
        Some(a) => a.to_string(),
        None => {
            if manifest.adapters.len() != 1 {
                return Err(AenvError::ManifestInvalid(format!(
                    "namespace '{namespace}' declares {} adapters; use --adapter to disambiguate",
                    manifest.adapters.len()
                )));
            }
            manifest.adapters.keys().next().unwrap().clone()
        }
    };

    // Derive a skill name from the source: last path component (for local) or
    // the URL fragment (for git#ref) or registry name. Users can rename by
    // editing the manifest if they don't like it.
    let skill_name = derive_skill_name(source).ok_or_else(|| {
        AenvError::ManifestInvalid(format!(
            "could not derive a skill name from source '{source}'; \
             pick a different source or edit the manifest manually"
        ))
    })?;

    if manifest.skills.iter().any(|s| s.name == skill_name) {
        return Err(AenvError::ManifestInvalid(format!(
            "namespace '{namespace}' already declares a skill '{skill_name}'"
        )));
    }

    let mut decl = SkillDecl {
        name: skill_name.clone(),
        mode: SkillMode::Imported,
        adapter: Some(adapter_name),
        source: Some(source.to_string()),
        ref_: pin.map(String::from),
        required: false,
    };

    // If --pin was specified, resolve to verify reachability + write the
    // resolved ref. If the user said `--pin master`, we want the actual SHA,
    // not the branch name. Use `apply_required_rule` with required=true so
    // resolution failure surfaces as an error.
    if pin.is_some() {
        decl.required = true;
        let resolution = apply_required_rule(fs, layout, &decl)?
            .expect("required=true should propagate errors");
        decl.required = false;
        if let Some(sha) = resolution.resolved_ref {
            decl.ref_ = Some(sha);
        }
    }

    let _ = adapters; // declarations don't need adapter lookup yet
    manifest.skills.push(decl);
    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    println!("Imported skill '{skill_name}' into namespace '{namespace}'");
    Ok(())
}

fn derive_skill_name(source: &str) -> Option<String> {
    if let Some(rest) = source.strip_prefix("git+") {
        if let Some((_, after_hash)) = rest.split_once('#') {
            return Some(after_hash.to_string());
        }
        let url_tail = rest.rsplit('/').next()?;
        return Some(url_tail.trim_end_matches(".git").to_string());
    }
    if let Some(name) = source.strip_prefix("registry:") {
        return Some(name.to_string());
    }
    // Local path: use last component.
    std::path::Path::new(source)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}
```

Wire into `cmd/skill/mod.rs`:

```rust
pub mod import;
```

In `main.rs`, add to the `SkillAction` enum:

```rust
/// Import a skill from a local path, git URL, or registry.
Import {
    /// Source: /abs/path, ~/path, git+URL[#ref], or registry:<name>.
    source: String,
    #[arg(long)]
    ns: String,
    #[arg(long)]
    adapter: Option<String>,
    #[arg(long)]
    pin: Option<String>,
},
```

And the dispatch:

```rust
SkillAction::Import { source, ns, adapter, pin } => {
    let adapters_reg =
        aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir())?;
    cmd::skill::import::run(
        &fs,
        &layout,
        &adapters_reg,
        &ns,
        &source,
        adapter.as_deref(),
        pin.as_deref(),
    )
}
```

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test skill_import_local_e2e --test skill_import_git_e2e 2>&1 | tail -10`
Expected: PASS — 2 tests pass; git test prints "skipping" when git absent.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-cli/src/cmd/skill/import.rs crates/aenv-cli/src/cmd/skill/mod.rs crates/aenv-cli/src/main.rs crates/aenv-cli/tests/skill_import_local_e2e.rs crates/aenv-cli/tests/skill_import_git_e2e.rs
git commit -m "Add 'aenv skill import' command"
```

---

### Task 14: `aenv skill list` command

Text-table output across all namespaces (or one if `--ns`). Columns: ENV, SKILL, MODE, SOURCE, PIN. Matches functional spec §5.11.

**Files:**
- Create: `crates/aenv-cli/src/cmd/skill/list.rs`
- Modify: `crates/aenv-cli/src/cmd/skill/mod.rs` (add `pub mod list;`)
- Modify: `crates/aenv-cli/src/main.rs` — add `List` variant to `SkillAction`
- Test: `crates/aenv-cli/tests/skill_list_e2e.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-cli/tests/skill_list_e2e.rs`:

```rust
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: std::path::PathBuf,
    project: std::path::PathBuf,
}
impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        let project = std::fs::canonicalize(project_guard.path()).unwrap();
        Self { _aenv_home_guard: aenv_home_guard, _project_guard: project_guard, aenv_home, project }
    }
    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
        c.env("AENV_HOME", &self.aenv_home);
        c
    }
    fn aenv_home(&self) -> &Path { &self.aenv_home }
    fn project(&self) -> &Path { &self.project }
}

#[test]
fn list_prints_all_skills_across_namespaces() {
    let h = Harness::new();
    h.cmd().args(["create", "experiments"]).output().unwrap();
    h.cmd().args(["create", "detailed-execution"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/experiments/aenv.toml"),
        r#"
name = "experiments"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "compare-approaches"
mode = "authored"
adapter = "claude-code"
"#,
    )
    .unwrap();
    std::fs::write(
        h.aenv_home().join("envs/detailed-execution/aenv.toml"),
        r#"
name = "detailed-execution"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "write-tests"
mode = "authored"
adapter = "claude-code"

[[skills]]
name = "match-conventions"
mode = "imported"
adapter = "claude-code"
source = "git+https://example.com/skills.git#match-conventions"
ref = "v1.2.0"
"#,
    )
    .unwrap();

    let out = h.cmd().args(["skill", "list"]).output().unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("experiments"));
    assert!(stdout.contains("compare-approaches"));
    assert!(stdout.contains("write-tests"));
    assert!(stdout.contains("match-conventions"));
    assert!(stdout.contains("v1.2.0"));
    assert!(stdout.contains("authored"));
    assert!(stdout.contains("imported"));
}

#[test]
fn list_filters_by_ns() {
    let h = Harness::new();
    h.cmd().args(["create", "a"]).output().unwrap();
    h.cmd().args(["create", "b"]).output().unwrap();
    std::fs::write(
        h.aenv_home().join("envs/a/aenv.toml"),
        "name = \"a\"\n\n[[skills]]\nname = \"alpha\"\nmode = \"authored\"\n",
    )
    .unwrap();
    std::fs::write(
        h.aenv_home().join("envs/b/aenv.toml"),
        "name = \"b\"\n\n[[skills]]\nname = \"beta\"\nmode = \"authored\"\n",
    )
    .unwrap();

    let out = h.cmd().args(["skill", "list", "--ns", "a"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("alpha"));
    assert!(!stdout.contains("beta"));
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test skill_list_e2e 2>&1 | tail -10`
Expected: FAIL — `skill list` doesn't exist.

- [ ] **Step 3: Implement**

Create `crates/aenv-cli/src/cmd/skill/list.rs`:

```rust
//! `aenv skill list [--ns <ns>]` — text-table output of every skill.

use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::SkillMode;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    ns_filter: Option<&str>,
) -> Result<()> {
    let envs_dir = layout.namespaces_dir();
    let namespaces: Vec<String> = if !fs.exists(&envs_dir)? {
        Vec::new()
    } else {
        let mut names: Vec<String> = fs
            .list_dir(&envs_dir)?
            .into_iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
            .filter(|name| ns_filter.map(|f| f == name).unwrap_or(true))
            .collect();
        names.sort();
        names
    };

    println!(
        "{:<20}  {:<30}  {:<10}  {:<60}  {}",
        "ENV", "SKILL", "MODE", "SOURCE", "PIN"
    );
    for ns in &namespaces {
        let manifest_path = layout.manifest_path(ns);
        if !fs.exists(&manifest_path)? {
            continue;
        }
        let bytes = fs.read(&manifest_path)?;
        let text = std::str::from_utf8(&bytes).unwrap_or("");
        let manifest = match AenvManifest::from_toml(text) {
            Ok(m) => m,
            Err(_) => continue,
        };
        for s in &manifest.skills {
            let mode = match s.mode {
                SkillMode::Authored => "authored",
                SkillMode::Imported => "imported",
            };
            let source = s.source.as_deref().unwrap_or("-");
            let pin = s.ref_.as_deref().unwrap_or("-");
            println!(
                "{:<20}  {:<30}  {:<10}  {:<60}  {}",
                ns, s.name, mode, source, pin
            );
        }
    }
    Ok(())
}
```

Add to `cmd/skill/mod.rs`:

```rust
pub mod list;
```

In `main.rs`, add to `SkillAction`:

```rust
/// List every skill in every namespace (or one if --ns).
List {
    #[arg(long)]
    ns: Option<String>,
},
```

And the dispatch:

```rust
SkillAction::List { ns } => cmd::skill::list::run(&fs, &layout, ns.as_deref()),
```

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test skill_list_e2e 2>&1 | tail -10`
Expected: PASS — 2 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-cli/src/cmd/skill/list.rs crates/aenv-cli/src/cmd/skill/mod.rs crates/aenv-cli/src/main.rs crates/aenv-cli/tests/skill_list_e2e.rs
git commit -m "Add 'aenv skill list' command"
```

---

### Task 15: Adapter `soft_limits` field + built-in defaults (R-25)

Each adapter declares per-role character soft limits. Phase 4 only uses `"instructions"`. Built-in defaults:

| Adapter | `soft_limits.instructions` |
|---|---|
| claude-code | 5000 |
| cursor | 5000 |
| cline | 5000 |
| continue | 5000 |
| aider | 5000 |
| windsurf | 6000 |
| mcp | (no instructions role; field absent) |

**Files:**
- Modify: `crates/aenv-core/src/adapter.rs` — add `soft_limits: BTreeMap<String, usize>` field
- Modify: `crates/aenv-core/src/adapters_builtin/{claude_code,cursor,cline,continue_,aider,windsurf}.toml`
- Test: `crates/aenv-core/tests/adapter_soft_limits.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/adapter_soft_limits.rs`:

```rust
use aenv_core::adapter::Adapter;

#[test]
fn parses_soft_limits() {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md"]

[soft_limits]
instructions = 5000
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert_eq!(a.soft_limits.get("instructions"), Some(&5000));
}

#[test]
fn missing_block_is_empty_map() {
    let toml = r#"
name = "x"
files = ["a"]
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert!(a.soft_limits.is_empty());
}

#[test]
fn builtins_declare_expected_limits() {
    use aenv_core::adapters_builtin::ALL;
    let mut found_claude = false;
    let mut found_windsurf = false;
    for (name, toml) in ALL {
        let adapter = Adapter::from_toml(toml).unwrap();
        match *name {
            "claude-code" => {
                assert_eq!(adapter.soft_limits.get("instructions"), Some(&5000));
                found_claude = true;
            }
            "windsurf" => {
                assert_eq!(adapter.soft_limits.get("instructions"), Some(&6000));
                found_windsurf = true;
            }
            "cursor" | "cline" | "continue" | "aider" => {
                assert_eq!(
                    adapter.soft_limits.get("instructions"),
                    Some(&5000),
                    "{name} should declare instructions=5000"
                );
            }
            "mcp" => {
                assert!(
                    adapter.soft_limits.get("instructions").is_none(),
                    "mcp has no instructions role"
                );
            }
            other => panic!("unexpected built-in adapter '{other}'"),
        }
    }
    assert!(found_claude);
    assert!(found_windsurf);
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test adapter_soft_limits 2>&1 | tail -10`
Expected: FAIL — `Adapter::soft_limits` doesn't exist.

- [ ] **Step 3: Extend `Adapter`**

In `crates/aenv-core/src/adapter.rs`, add to the struct:

```rust
/// Per-role character soft limits. Currently used only for the
/// "instructions" role (R-24 / R-25). Empty for adapters that don't
/// declare any role with a size guard.
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub soft_limits: BTreeMap<String, usize>,
```

- [ ] **Step 4: Add `[soft_limits]` blocks to built-in adapters**

For each of `claude_code.toml`, `cursor.toml`, `cline.toml`, `continue_.toml`, `aider.toml`, append:

```toml

[soft_limits]
instructions = 5000
```

For `windsurf.toml`, append:

```toml

[soft_limits]
instructions = 6000
```

`mcp.toml` gets no change.

- [ ] **Step 5: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test adapter_soft_limits 2>&1 | tail -10`
Expected: PASS — 3 tests passed.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | tail -5`
Expected: workspace green. Any test that constructs an `Adapter` struct literal needs `soft_limits: BTreeMap::new()` added — find with `grep -rn "Adapter {" crates/`.

- [ ] **Step 6: Commit**

```bash
git add -u
git add crates/aenv-core/tests/adapter_soft_limits.rs
git commit -m "Add adapter soft_limits field + built-in defaults (R-25)"
```

---

### Task 16: `instructions_budget` parameter declarations + integer min combiner (R-26)

The six text-instruction adapters declare:

```toml
[[parameters]]
name = "instructions_budget"
type = "integer"
```

This lets R-26's "effective limit = min(adapter default, namespace budget)" work via Phase 3's existing parameter pipeline.

The `instructions_max_chars` evaluator gains a small change: instead of using `policy.value` as the limit directly, it uses `min(policy_value, instructions_budget_param)` when the param is present in the resolved parameters.

**Files:**
- Modify: `crates/aenv-core/src/adapters_builtin/{claude_code,cursor,cline,continue_,aider,windsurf}.toml`
- Modify: `crates/aenv-core/src/policies/builtin/instructions_max_chars.rs`
- Test: `crates/aenv-core/tests/doctor_instructions_budget_narrows.rs`

- [ ] **Step 1: Add the parameter declaration to built-in adapters**

For each of `claude_code.toml`, `cursor.toml`, `cline.toml`, `continue_.toml`, `aider.toml`, `windsurf.toml`, append:

```toml

[[parameters]]
name = "instructions_budget"
type = "integer"
```

- [ ] **Step 2: Write the failing test**

Create `crates/aenv-core/tests/doctor_instructions_budget_narrows.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::doctor::evaluate;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::parameters::{ParameterValue, ResolvedParameter};
use aenv_core::policies::builtin::OutcomeStatus;
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

#[test]
fn instructions_budget_narrows_effective_limit() {
    let fs = MockFilesystem::new();
    let body = "x".repeat(4000);
    fs.write(&PathBuf::from("/h/envs/base/CLAUDE.md"), body.as_bytes()).unwrap();

    let mut adapters = AdapterRegistry::new();
    let mut roles = BTreeMap::new();
    roles.insert("CLAUDE.md".into(), "instructions".into());
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
        skills_dir: Some(".claude/skills".into()),
        soft_limits: BTreeMap::from([("instructions".into(), 5000usize)]),
    });
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    let mut parameters: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    parameters.insert(
        "instructions_budget".into(),
        ResolvedParameter {
            value: ParameterValue::Integer(3000),
            source: ns("base"),
        },
    );

    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
            skill_provenance: None,
        }],
        parameters,
        policies: BTreeMap::from([(
            "instructions_max_chars".into(),
            ResolvedPolicy {
                value: PolicyValue::Integer(5000),
                enforce: false,
                source: ns("base"),
            },
        )]),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    // 4000 chars > 3000 effective limit (budget narrows from 5000 to 3000).
    let fails: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
        .collect();
    assert!(
        !fails.is_empty(),
        "expected a warning (effective limit=3000, body=4000); got {:?}",
        report.outcomes
    );
}
```

- [ ] **Step 3: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test doctor_instructions_budget_narrows 2>&1 | tail -10`
Expected: FAIL — the evaluator currently uses the raw `policy.value` (5000) and 4000 chars passes.

- [ ] **Step 4: Update the evaluator**

In `crates/aenv-core/src/policies/builtin/instructions_max_chars.rs`, change the body of `evaluate` so the limit calculation considers `instructions_budget`:

```rust
let policy_limit = match &policy.value {
    PolicyValue::Integer(n) if *n >= 0 => *n as usize,
    _ => {
        return vec![PolicyOutcome::warn_skip(
            KEY,
            format!(
                "policy '{KEY}' must be a non-negative integer; got {} (source: {})",
                policy.value.type_tag(),
                policy.source
            ),
        )];
    }
};
let budget_limit = match ctx.resolved.parameters.get("instructions_budget") {
    Some(rp) => match &rp.value {
        ParameterValue::Integer(n) if *n >= 0 => Some(*n as usize),
        _ => None,
    },
    None => None,
};
let effective = match budget_limit {
    Some(b) => policy_limit.min(b),
    None => policy_limit,
};
```

Then use `effective` everywhere `limit` was used. Update the message to clarify when the budget narrowed the limit (`format!(" (effective {effective}; policy {policy_limit}, budget {budget_limit:?})")` or similar).

You'll need to import `ParameterValue` from `crate::parameters`.

- [ ] **Step 5: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test doctor_instructions_budget_narrows --test policy_instructions_max_chars 2>&1 | tail -10`
Expected: PASS — old tests still pass; new test passes.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | tail -5`
Expected: workspace green.

- [ ] **Step 6: Commit**

```bash
git add -u
git add crates/aenv-core/tests/doctor_instructions_budget_narrows.rs
git commit -m "Add instructions_budget parameter + narrow effective limit (R-26)"
```

---

### Task 17: Auto-fire `instructions_max_chars` when no manifest declares it (R-24)

R-24's EARS trigger says "WHILE a namespace contains instructions files, the system shall warn… when activation would materialize an instructions file exceeding the adapter's documented soft limit." Today, the policy fires only if a manifest declares `instructions_max_chars`. This task makes it fire unconditionally when:

1. At least one resolved candidate has `role = "instructions"` (per its adapter).
2. The adapter has a `soft_limits.instructions` entry.
3. The manifest did NOT declare `instructions_max_chars` (otherwise the declared policy takes precedence).

The synthesis happens inside `doctor::evaluate` before dispatch. The synthesized `ResolvedPolicy` uses the adapter's `soft_limits.instructions` as its value; the existing evaluator then narrows with `instructions_budget` from Task 16.

For multi-adapter namespaces with different soft limits (e.g. claude-code = 5000, windsurf = 6000), we synthesize the policy at the LOWER value — the strictest one applies across all instructions files. This is conservative; per-file customization can land later.

**Files:**
- Modify: `crates/aenv-core/src/doctor.rs` — `evaluate` adds the auto-fire step
- Test: `crates/aenv-core/tests/doctor_auto_instructions_limit.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/doctor_auto_instructions_limit.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::doctor::evaluate;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::OutcomeStatus;
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

#[test]
fn auto_fires_when_manifest_silent_and_oversized() {
    let fs = MockFilesystem::new();
    let body = "x".repeat(8000);
    fs.write(&PathBuf::from("/h/envs/base/CLAUDE.md"), body.as_bytes()).unwrap();

    let mut adapters = AdapterRegistry::new();
    let mut roles = BTreeMap::new();
    roles.insert("CLAUDE.md".into(), "instructions".into());
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
        skills_dir: Some(".claude/skills".into()),
        soft_limits: BTreeMap::from([("instructions".into(), 5000usize)]),
    });
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
            skill_provenance: None,
        }],
        parameters: BTreeMap::new(),
        // CRITICAL: no policy declared. R-24 says we should still warn.
        policies: BTreeMap::new(),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    // Expect a Warn outcome from the synthesized policy.
    let warns: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
        .collect();
    assert!(
        !warns.is_empty(),
        "expected R-24 auto-warn for 8000-char file; got {:?}",
        report.outcomes
    );
    // Synthesized policy appears in the report.
    assert!(
        report.policies.contains_key("instructions_max_chars"),
        "expected synthesized instructions_max_chars in report.policies"
    );
}

#[test]
fn does_not_fire_when_manifest_declares_explicitly() {
    use aenv_core::policies::{PolicyValue, ResolvedPolicy};
    let fs = MockFilesystem::new();
    let body = "x".repeat(8000);
    fs.write(&PathBuf::from("/h/envs/base/CLAUDE.md"), body.as_bytes()).unwrap();

    let mut adapters = AdapterRegistry::new();
    let mut roles = BTreeMap::new();
    roles.insert("CLAUDE.md".into(), "instructions".into());
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
        skills_dir: Some(".claude/skills".into()),
        soft_limits: BTreeMap::from([("instructions".into(), 5000usize)]),
    });
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    let mut policies = BTreeMap::new();
    policies.insert(
        "instructions_max_chars".into(),
        ResolvedPolicy {
            value: PolicyValue::Integer(10_000), // looser than adapter default
            enforce: false,
            source: ns("base"),
        },
    );
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
            skill_provenance: None,
        }],
        parameters: BTreeMap::new(),
        policies,
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    // The manifest-declared 10_000 limit means 8000 is fine.
    let warns: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
        .collect();
    assert!(
        warns.is_empty(),
        "manifest's 10_000 limit takes precedence; expected no warn; got {:?}",
        report.outcomes
    );
}

#[test]
fn does_not_fire_when_no_instructions_role_present() {
    let fs = MockFilesystem::new();
    let adapters = AdapterRegistry::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
    };
    let report = evaluate(&fs, &layout, &adapters, &resolved);
    assert!(
        !report.policies.contains_key("instructions_max_chars"),
        "no instructions files → no auto-fire"
    );
}
```

- [ ] **Step 2: Verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test doctor_auto_instructions_limit 2>&1 | tail -15`
Expected: FAIL — first test: no warn because no policy declared.

- [ ] **Step 3: Synthesize the policy in `doctor::evaluate`**

In `crates/aenv-core/src/doctor.rs`, modify `evaluate` to inject a synthesized policy when the conditions are met. Before the dispatch loop:

```rust
// Build a mutable copy of policies so we can inject the R-24 auto-fire.
let mut effective_policies = resolved.policies.clone();
if !effective_policies.contains_key("instructions_max_chars") {
    // Find the strictest soft_limits.instructions across all adapters that
    // own at least one instructions-role candidate.
    let mut min_limit: Option<usize> = None;
    for c in &resolved.candidates {
        let adapter = match adapters.get(&c.adapter) {
            Some(a) => a,
            None => continue,
        };
        let role = adapter
            .roles
            .get(c.path.to_string_lossy().as_ref())
            .map(String::as_str)
            .unwrap_or("");
        if role != "instructions" {
            continue;
        }
        if let Some(&limit) = adapter.soft_limits.get("instructions") {
            min_limit = Some(min_limit.map_or(limit, |m| m.min(limit)));
        }
    }
    if let Some(limit) = min_limit {
        let leaf = resolved
            .chain
            .last()
            .cloned()
            .unwrap_or_else(|| crate::identity::NamespaceId::new("(synthesized)").unwrap());
        effective_policies.insert(
            "instructions_max_chars".to_string(),
            crate::policies::ResolvedPolicy {
                value: crate::policies::PolicyValue::Integer(limit as i64),
                enforce: false,
                source: leaf,
            },
        );
    }
}
```

Then iterate `effective_policies` instead of `resolved.policies` in the dispatch loop, and use `effective_policies` (not `resolved.policies`) in the `DoctorReport` construction.

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test doctor_auto_instructions_limit 2>&1 | tail -15`
Expected: PASS — 3 tests passed.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | tail -5`
Expected: workspace green.

- [ ] **Step 5: Commit**

```bash
git add -u
git add crates/aenv-core/tests/doctor_auto_instructions_limit.rs
git commit -m "Auto-fire instructions_max_chars from adapter soft_limits (R-24)"
```

---

### Task 18: End-to-end integration test (spec §5.9, §5.10, §5.11) + fmt/clippy sweep

A binary-level test that reproduces the spec's three skill journeys, then a final cargo fmt + clippy --workspace --all-targets -- -D warnings + cargo test --workspace sweep.

**Files:**
- Create: `crates/aenv-cli/tests/skills_lifecycle_e2e.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-cli/tests/skills_lifecycle_e2e.rs`:

```rust
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: std::path::PathBuf,
    project: std::path::PathBuf,
}
impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        let project = std::fs::canonicalize(project_guard.path()).unwrap();
        Self { _aenv_home_guard: aenv_home_guard, _project_guard: project_guard, aenv_home, project }
    }
    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
        c.env("AENV_HOME", &self.aenv_home);
        c
    }
    fn aenv_home(&self) -> &Path { &self.aenv_home }
    fn project(&self) -> &Path { &self.project }
}

fn assert_success(label: &str, out: &std::process::Output) {
    assert!(
        out.status.success(),
        "{label} failed: status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn full_skills_lifecycle_matches_spec() {
    let h = Harness::new();

    // Spec §5.9: author a skill.
    assert_success("create", &h.cmd().args(["create", "detailed-execution"]).output().unwrap());
    std::fs::write(
        h.aenv_home().join("envs/detailed-execution/aenv.toml"),
        "name = \"detailed-execution\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(h.aenv_home().join("envs/detailed-execution/CLAUDE.md"), "hi").unwrap();
    let out = h
        .cmd()
        .args(["skill", "new", "run-migration", "--ns", "detailed-execution"])
        .output()
        .unwrap();
    assert_success("skill new", &out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Created authored skill"));
    assert!(
        h.aenv_home()
            .join("envs/detailed-execution/.claude/skills/run-migration/SKILL.md")
            .exists()
    );

    // Spec §5.10: import a local skill.
    let external = tempdir().unwrap();
    std::fs::write(
        external.path().join("SKILL.md"),
        "---\nname: check-before-submit\ndescription: y\n---\n",
    )
    .unwrap();
    let canonical = std::fs::canonicalize(external.path()).unwrap();
    let out = h
        .cmd()
        .args(["skill", "import"])
        .arg(canonical.to_str().unwrap())
        .args(["--ns", "detailed-execution", "--adapter", "claude-code"])
        .output()
        .unwrap();
    assert_success("skill import local", &out);

    // Spec §5.11: list skills.
    let out = h.cmd().args(["skill", "list"]).output().unwrap();
    assert_success("skill list", &out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("run-migration"));
    assert!(stdout.contains("check-before-submit"));
    assert!(stdout.contains("authored"));
    assert!(stdout.contains("imported"));

    // Activation works end-to-end.
    assert_success(
        "use",
        &h.cmd().args(["use", "detailed-execution"]).current_dir(h.project()).output().unwrap(),
    );
    assert_success(
        "activate",
        &h.cmd().args(["activate"]).current_dir(h.project()).output().unwrap(),
    );
    assert!(h.project().join(".claude/skills/run-migration/SKILL.md").exists());
    let imported_path = h
        .project()
        .join(format!(".claude/skills/{}/SKILL.md", canonical.file_name().unwrap().to_string_lossy()));
    assert!(
        imported_path.exists(),
        "expected imported skill at {} (project={})",
        imported_path.display(),
        h.project().display()
    );

    // Status shows both.
    let out = h.cmd().args(["status"]).current_dir(h.project()).output().unwrap();
    assert_success("status", &out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Skills"));
    assert!(stdout.contains("run-migration"));
}
```

- [ ] **Step 2: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test skills_lifecycle_e2e 2>&1 | tail -20`
Expected: PASS — 1 test passed.

- [ ] **Step 3: Run cargo fmt**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo fmt`
Expected: silent.

Run: `git status` — expect formatting changes across Phase 4 files. Stage them.

- [ ] **Step 4: Run cargo clippy with `-D warnings`**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20`
Expected: silent (no errors). Common Phase 4-introduced themes to watch for:
- `clippy::let_and_return` in evaluator helpers
- `clippy::redundant_closure` on `or_else` chains
- `clippy::needless_borrow` on path arguments

Fix any warnings inline.

- [ ] **Step 5: Run full workspace tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | tail -5`
Expected: all tests pass (~360 after Phase 4 additions).

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-cli/tests/skills_lifecycle_e2e.rs
git add -u
git commit -m "Add end-to-end skills-lifecycle integration test + cargo fmt sweep"
```

---

### Task 19: Tag `phase-4-complete`

Final mile-marker.

- [ ] **Step 1: Final regression**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -5`
Expected: every test passes.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5`
Expected: silent.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check`
Expected: silent.

- [ ] **Step 2: Tag**

```bash
git tag -a phase-4-complete -m "$(cat <<'EOF'
Phase 4 complete: skills lifecycle + instructions defaults

Deliverable:
- [[skills]] manifest table with authored / imported modes.
- SourceKind parser for local paths, git URLs (git+...#ref), and a
  Registry stub.
- Cache layout: ~/.aenv/cache/skills/<source-hash>/<ref>/.
- Git wrapper (ls-remote, clone --depth 1, rev-parse HEAD) with
  availability probe; failures map to RemoteUnreachable (exit 14).
- required = true → activation aborts with ActivationConflict (exit 13)
  if the source is unreachable; default warns and skips.
- CLI: aenv skill new / import [--pin] / list.
- aenv status prints Skills section grouped by mode + source + pin.
- State schema bumped to 4; schema 3 still reads (skill_provenance is
  defaulted to None).
- R-24/R-25/R-26 deferred from Phase 3 also landed: each built-in
  adapter declares soft_limits.instructions (5000 general / 6000
  windsurf); instructions_budget parameter narrows the effective limit
  to min(adapter, ns); instructions_max_chars auto-fires when no
  manifest declares it.

Covers PRD: R-14, R-15, R-16 (Registry deferred), R-17, R-18, R-19,
R-20, R-21, R-22, R-23, R-24, R-25, R-26.

Covers functional spec: §5.9, §5.10, §5.11 (text-table flavor; --json
lands in Phase 5).

Deliberately deferred to later phases:
- Adapter parameter projection (R-68 second half) — design pending.
- SourceKind::Registry resolution — pending registry design.
- aenv skill refresh — re-fetch implicit on each activation for now.
- --json on every read-oriented command (Phase 5).
EOF
)"
```

- [ ] **Step 3: Verify tag**

Run: `git tag -l --format='%(contents:subject)' phase-4-complete`
Expected: `Phase 4 complete: skills lifecycle + instructions defaults`.

Run: `git log --oneline phase-3-complete..phase-4-complete | wc -l`
Expected: ~19–20 commits (one per task plus the plan-add commit if you committed the plan separately).

---

## Phase 4 completion check

After Task 19:

- [ ] Every checkbox in this plan is checked.
- [ ] `cargo test --workspace --all-targets` is green.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is silent.
- [ ] `cargo fmt --check` is silent.
- [ ] `phase-4-complete` git tag points at the final commit.
- [ ] State schema is 4; an existing schema-3 state file still loads.
- [ ] PRD requirements R-14 through R-26 all have a corresponding test that exercises the requirement (R-16 partial: Registry stubbed).
- [ ] Functional spec §5.9 (`aenv skill new`), §5.10 (`aenv skill import`), §5.11 (`aenv skill list`) reproduced end-to-end by `skills_lifecycle_e2e.rs`.
- [ ] `aenv status` Skills section displays both authored and imported skills.
- [ ] An oversized CLAUDE.md triggers a doctor warning even when no manifest declares `instructions_max_chars`.

If any criterion fails, fix it in a follow-up commit and re-tag (delete + recreate `phase-4-complete`).
