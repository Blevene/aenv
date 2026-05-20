# aenv Implementation Roadmap

> **For agentic workers:** This is a *milestone-level* roadmap, not a bite-sized task list. Each phase becomes its own detailed plan via `superpowers:writing-plans` when its turn comes — the detail-now-vs-defer choice avoids speculating about Phase 5 before Phase 2 has settled.

**Goal:** Build aenv from scratch into a polished, scriptable Rust CLI for AI-coding-harness configuration, per `pm_docs/aenv-prd.md` v0.3, `pm_docs/aenv-functional-spec.md` v0.3, and `pm_docs/aenv-engineering.md` v0.2.

**Architecture:** Single-binary Rust CLI. `clap` v4 derive for parsing, `serde`+`toml`+`serde_json`+`serde_yaml` for data, `sha2` for hashing, `thiserror` for errors, `tempfile` + `proptest` + `insta` for tests. `Filesystem` trait isolates I/O for testability. Shells out to system `git` for remote operations. Public contracts: `::` (artifacts) / `.` (parameters) separators, exit codes 10–20, resolved-namespace hash (`sha256-v1:<hex>`), JSON output schemas.

**Tech stack:** Rust (stable), cargo workspaces, GitHub Actions CI on Linux + macOS + Windows.

**Sequencing principle:** Each phase produces shippable software for *some* real use case, and a user could stop at any phase boundary with a coherent tool. Phase 1 alone is a working single-namespace activator; later phases add capability without breaking earlier ones.

---

## Phase 0 — Project skeleton

**Deliverable:** Cargo workspace compiles, CI green, no user-visible features beyond `aenv --version`.

**Why this phase exists:** Lock in the foundational types (`AenvError` enum, `Filesystem` trait) and CI before writing any feature code. Both are referenced by every later phase; getting them right now costs less than retrofitting.

**Files to create:**
- `Cargo.toml` (workspace root)
- `crates/aenv-core/Cargo.toml` (library)
- `crates/aenv-core/src/lib.rs`
- `crates/aenv-core/src/error.rs` — `AenvError` enum per engineering §3
- `crates/aenv-core/src/fs.rs` — `Filesystem` trait + `RealFilesystem` + `MockFilesystem`
- `crates/aenv-cli/Cargo.toml` (binary)
- `crates/aenv-cli/src/main.rs` — clap skeleton, `--version` only
- `.github/workflows/ci.yml`
- `rustfmt.toml`, `clippy.toml`
- `README.md` (minimal — "see pm_docs/")
- `.gitignore` (`target/`, `.aenv/` test artifacts)

**Phase completion criteria (all must be true):**
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` succeeds (with zero tests, just compiles)
- [ ] `cargo clippy --workspace -- -D warnings` succeeds
- [ ] `cargo fmt --check` succeeds
- [ ] CI matrix green on Linux + macOS (Windows deferred to Phase 7)
- [ ] `aenv --version` prints crate version
- [ ] `AenvError` has all variants from engineering §3 with documented exit codes
- [ ] `Filesystem` trait has ~12 methods per engineering §5; `MockFilesystem` is a programmable in-memory implementation
- [ ] First commit on `main` is green

**PRD coverage:** None directly; lays groundwork for R-82 (exit codes) and engineering §5 (testability).

---

## Phase 1 — Single-namespace happy path

**Deliverable:** A user can create a namespace with one adapter (claude-code), pin it to a project, activate it, observe files materialize as symlinks with displaced originals backed up, deactivate, and restore. No composition, no parameters, no policies.

**Why this phase exists:** Validates the file-materialization core — backup, symlink, state tracking, rollback — against real filesystems. Everything else in aenv hangs off this primitive working correctly.

**Files to create (in `aenv-core`):**
- `src/manifest.rs` — `aenv.toml` parsing
- `src/adapter.rs` — adapter TOML parsing + registry
- `src/adapters_builtin/claude_code.toml` — embedded via `include_str!`
- `src/namespace.rs` — registry layout, `aenv create/list/delete`
- `src/project.rs` — `.aenv` pin file, project-root walk
- `src/state.rs` — `.aenv/state.json` schema (with `schema_version: 1`)
- `src/activate.rs` — materialization + backup + rollback
- `src/atomicity.rs` — rename-probe per engineering §7

**Files to create (in `aenv-cli`):**
- `src/cmd/create.rs`, `list.rs`, `delete.rs`, `use_.rs`, `activate.rs`, `deactivate.rs`, `status.rs`, `restore.rs`
- Wire `--project <path>` flag through every command

**Files to create (in `tests/`):**
- `tests/common/mod.rs` — `TestEnv` harness using `tempfile::tempdir()`
- `tests/lifecycle.rs` — create / list / delete
- `tests/activation.rs` — activate / deactivate / restore round trip
- `tests/conflicts.rs` — backup-on-displace, byte-identical no-op, rollback on partial failure

**Phase completion criteria:**
- [ ] `aenv create <name>` scaffolds a namespace directory with default `aenv.toml`
- [ ] `aenv list` enumerates the registry (text only — `--json` lands in Phase 5)
- [ ] `aenv delete <name>` refuses if the namespace is currently active in a tracked project, otherwise removes
- [ ] `aenv use <name>` writes `.aenv` file at project root
- [ ] `aenv activate` materializes via symlink for new files; backs up displaced files to `.aenv/backup/<ISO-timestamp>/`; leaves byte-identical files in place and marks them managed
- [ ] `aenv deactivate` removes only files it materialized; restores backups; leaves user-created files alone
- [ ] `aenv restore` restores the most recent backup set even when no namespace is active
- [ ] `aenv status` prints text-only status: active namespace, managed files
- [ ] `--project <path>` flag accepted by every command; library never reads `std::env::current_dir()`
- [ ] Atomicity rename-probe runs at activation start; cross-filesystem boundary aborts with `ActivationConflict`
- [ ] Activation failure mid-way triggers full rollback to pre-activation state
- [ ] Exit codes wired: 1 generic, 10 namespace-not-found, 11 adapter-missing, 12 manifest-invalid, 13 activation-conflict, 20 project-not-pinned
- [ ] Integration tests exercise each happy-path journey and the rollback case

**PRD coverage:** R-1, R-2, R-3, R-4, R-5, R-29 (one adapter), R-30 (one of seven built-ins), R-31, R-32, R-33, R-34, R-37, R-38, R-43, R-44, R-45, R-46, R-48, R-49 (defer Windows copy-mode to Phase 7), R-50 (partial — single-namespace flavor), R-60, R-61, R-62, R-63, R-78, R-79, R-82 (codes listed above), R-83.

**Functional spec coverage:** §5.1 first-time setup (`create`), §5.2 pinning, §5.4 partial (only single-namespace `cd` cases work — auto-activation defers to Phase 6).

---

## Phase 2 — Composition: extends chain, merging, namespace identity

**Deliverable:** Namespaces can `extends` other namespaces. Section-merge produces combined instructions files; deep-merge handles JSON/YAML/TOML. Qualified names and shadow chains thread through resolution end-to-end. `aenv which` shows provenance. The remaining six built-in adapters ship.

**Why this phase exists:** Composition is the conceptual heart of aenv. Until this phase lands, "namespace" is just "named directory of files." After it lands, the substrate that everything else hangs off — qualified names, shadow chains — is real.

**Files to create:**
- `aenv-core/src/identity.rs` — `NamespaceId`, `ShortName`, `QualifiedName` per engineering §7.5
- `aenv-core/src/resolve.rs` — `extends` chain resolution, cycle detection, `ResolvedNamespace` + `ResolvedArtifact`
- `aenv-core/src/merge/section.rs` — Markdown section-merge with `<!-- aenv:replace -->`
- `aenv-core/src/merge/deep_json.rs` — `serde_json::Value` deep-merge
- `aenv-core/src/merge/deep_yaml.rs` — `serde_yaml` → JSON for merging
- `aenv-core/src/merge/deep_toml.rs` — `toml::Value` deep-merge
- `aenv-core/src/shadow.rs` — shadow-tracking data + lookup
- `aenv-core/src/adapters_builtin/{cursor,aider,cline,continue_,windsurf,mcp}.toml`
- `aenv-cli/src/cmd/which.rs`
- `aenv-cli/src/cmd/fork.rs` — both `aenv fork` (in-project) and `aenv fork <name>` (to new namespace)
- `tests/composition.rs` — extends chains, merge strategies, cycle detection
- `tests/shadow.rs` — overlay shadows parent, shadow chain preserved
- `tests/which.rs` — provenance queries

**Phase completion criteria:**
- [ ] `extends` resolution walks the chain depth-first; cycles abort with `ExtendsCycle` (exit 15)
- [ ] Section-merge default for `role = "instructions"` files; `<!-- aenv:replace -->` forces replace
- [ ] `merge = "deep"` for JSON/YAML/TOML produces merged file as regular (non-symlink) file
- [ ] Last-wins fallback for everything else
- [ ] Every emitted artifact carries a `QualifiedName`; the resolver returns `(qualified, shadows: Vec<QualifiedName>)`
- [ ] No materialized path on disk contains `::` (property-tested by walking the project tree post-activation)
- [ ] `aenv which <path>` prints qualified name, shadow chain, materialization strategy
- [ ] `aenv status` shows full resolution chain + per-file qualified provenance (text only)
- [ ] `aenv fork` replaces symlinks with copies in the project and marks it detached
- [ ] `aenv fork <name>` creates a new namespace populated from the current project state
- [ ] All seven built-in adapters ship, embedded via `include_str!`, with `aenv adapter list` reporting all
- [ ] Property tests: shadow preservation, qualified-name uniqueness within a namespace, materialized-path invariant

**PRD coverage:** R-6, R-7, R-8, R-9, R-10, R-11, R-12, R-13, R-30 (remaining adapters), R-47, R-50 (full version with chain + provenance), R-52, R-53, R-54.

**Functional spec coverage:** §5.3 switching harnesses (now actually meaningful), §5.5 provenance queries (text), §5.7 forking, the `experiments`/`detailed-execution`/`analyst` example namespaces should be authorable by hand and activate correctly.

---

## Phase 3 — Parameters & policies

**Deliverable:** Manifests declare typed parameters and validation policies. Inheritance flows from parent to child with documented semantics. `aenv get`, `aenv set`, `aenv doctor` work. `enforce = true` policies block activation.

**Why this phase exists:** Parameters and policies are what turn a namespace from "bundle of files" into "configurable, validated harness identity." The functional spec example manifests (`detailed-execution` overriding `default_model`, `forbid_tools` for `analyst`) depend on this phase landing.

**Files to create:**
- `aenv-core/src/parameters.rs` — typed parameter parsing (string/int/bool/list-of-string), inheritance, type-compat checking
- `aenv-core/src/policies.rs` — policy parsing, inheritance with enforce-protection
- `aenv-core/src/policies/builtin/` — `instructions_max_chars`, `skill_requires_description`, `mcp_requires_command_or_url`, `forbid_paths`
- `aenv-core/src/doctor.rs` — policy evaluation, report generation
- `aenv-cli/src/cmd/{get,set,doctor}.rs`
- `tests/parameters.rs`, `tests/policies.rs`, `tests/doctor.rs`

**Phase completion criteria:**
- [ ] `[parameters]` block parses with the four supported types; type-incompatible values abort with `ManifestInvalid` (exit 12)
- [ ] Parameter inheritance is last-wins per-key across the `extends` chain; effective set is recorded in `state.json`
- [ ] `aenv get <ns>.<param>` prints value + which namespace in the chain supplied it
- [ ] `aenv set <ns>.<param> <value>` updates the named namespace's manifest
- [ ] Adapters declare which parameters they consume + how they project (declared in the adapter TOML)
- [ ] `[policies]` block parses; built-in policy keys validated; unknown keys ignored with a warning
- [ ] `enforce = true` violations abort `aenv activate` (exit 17)
- [ ] Child cannot remove or weaken a parent's `enforce = true` policy
- [ ] Soft size limits per adapter (5,000 chars general; 6,000 Windsurf) materialized as default policies
- [ ] `aenv doctor` walks the union of own + inherited policies and prints per-policy outcomes with qualified identities
- [ ] `aenv doctor` exit code is 0 unless an `enforce = true` policy is violated
- [ ] Exit codes 16 (parameter undefined), 17 (policy violation) wired
- [ ] Integration test for `experiments-overgrown`-style failure (functional spec §5.12)

**PRD coverage:** R-24, R-25, R-26, R-27, R-28, R-66, R-67, R-68, R-69, R-70, R-71, R-72, R-73, R-74, R-75.

**Functional spec coverage:** §5.5 parameter queries, §5.12 `aenv doctor` (both clean and violation cases).

---

## Phase 4 — Skills lifecycle: authored + imported

**Deliverable:** `aenv skill new` scaffolds authored skills; `aenv skill import` adds imported entries from local path or git URL with optional pinning. `aenv skill list` enumerates. Imported skills resolve at activation time and cache under `~/.aenv/cache/skills/`.

**Why this phase exists:** Skills are the load-bearing differentiation surface in the design (`pm_docs/aenv-functional-spec.md` §2 calls this out). The harness comparison story in functional spec §6 isn't meaningful until users can author and import skills cheaply.

**Files to create:**
- `aenv-core/src/skills.rs` — `[[skills]]` and `[[agents]]` manifest entries, authored vs imported discriminator
- `aenv-core/src/skills/source.rs` — `SourceKind::Local | SourceKind::Git | SourceKind::Registry` (Registry stubbed)
- `aenv-core/src/skills/cache.rs` — `~/.aenv/cache/skills/<source-hash>/<ref>/` layout
- `aenv-core/src/git.rs` — shell-out helpers (`Command::new("git")` for `clone --depth 1`, `ls-remote`, etc.) with availability probe
- `aenv-cli/src/cmd/skill/{new,import,list,refresh}.rs`
- `tests/skills_authored.rs`, `tests/skills_imported_local.rs`, `tests/skills_imported_git.rs`

**Phase completion criteria:**
- [ ] `aenv skill new <name> --ns <ns> --adapter <a>` creates SKILL.md with adapter-appropriate frontmatter (`name`, `description`) + manifest `[[skills]]` entry with `mode = "authored"`
- [ ] `aenv skill import <source> --ns <ns> [--pin <ref>]` adds an entry with `mode = "imported"`; resolves and writes pinned ref if `--pin`
- [ ] Local-path imports work
- [ ] Git imports work via shell-out; cached under `~/.aenv/cache/skills/<source-hash>/<ref>/`
- [ ] `aenv skill refresh` re-fetches unpinned imports; no-ops on pinned ones
- [ ] Imported skills materialize at activation; provenance (source, resolved ref, resolved content hash) recorded in `state.json`
- [ ] `required = true` makes unreachable imports fail activation (exit 13 activation-conflict, or a new dedicated code if we add one); default is warn + skip
- [ ] `aenv skill list [--ns <ns>]` text output covers mode + source + pinned ref
- [ ] Git availability probe fires only for commands that need git; surfaces clear `git not on PATH` message with exit 14
- [ ] `SourceKind::Registry` returns explicit "not yet implemented" error pointing at PRD open question
- [ ] Integration tests for both source types + pinning + failure modes

**PRD coverage:** R-14, R-15, R-16 (Registry deferred), R-17, R-18, R-19, R-20, R-21, R-22, R-23.

**Functional spec coverage:** §5.9, §5.10, §5.11 (text-table flavor; `--json` lands in Phase 5).

---

## Phase 5 — Resolved-namespace hash & scriptability

**Deliverable:** Every read-oriented command emits `--json` with stable schemas locked by snapshot tests. The resolved-namespace hash is computed and exposed per PRD §5.17. Property tests cover hash invariants; a cross-machine fixture test guards platform divergence.

**Why this phase exists:** The downstream eval-tool consumer the PRD §5.16/§5.17 designs for needs *all* of: stable JSON, qualified names everywhere in machine output, the hash, and predictable exit codes. We've been emitting the exit codes since Phase 1 — this phase delivers the rest.

**Files to create:**
- `aenv-core/src/hash.rs` — algorithm-version byte 0x01, length-prefixed serialization, SHA-256
- `aenv-core/src/jcs.rs` — RFC 8785 JSON Canonicalization Scheme (~150 lines per engineering §2)
- `aenv-core/src/json_schema/` — typed structs for every `--json` response shape
- Modify every `aenv-cli/src/cmd/*.rs` to accept `--json` for read commands
- `aenv-cli/src/cmd/diff.rs` — project drift diff + namespace-vs-namespace structural diff
- `tests/hash_properties.rs` — proptest properties from engineering §8.3
- `tests/json_snapshots.rs` — insta-locked schemas for every `--json` shape
- `tests/fixtures/cross_machine/` — checked-in serialized namespaces + expected hashes per engineering §8.5

**Phase completion criteria:**
- [ ] RFC 8785 JCS implementation passes the spec's standard test vectors
- [ ] Hash computation follows R-84 exactly: extends-resolution → canonicalize structured files → append `.aenv/parameters.json` → sort byte-wise lex → length-prefix → prepend algorithm byte 0x01 → SHA-256
- [ ] Hash exposed as `sha256-v1:<lowercase-hex>` in `status --json` and `list --json`
- [ ] `--json` flag works on: `status`, `list`, `which`, `diff`, `adapter list`, `skill list`, `get`, `doctor`
- [ ] All `--json` output uses qualified names; short names included as a separate field where adapter consumption matters
- [ ] `aenv diff` (project drift): per-file unified diff for managed files that have drifted from the resolved namespace
- [ ] `aenv diff <ns-a> <ns-b>`: structural diff covering skills, agents, parameters, instructions sections (text + `--json`)
- [ ] Property tests cover: order independence, whitespace invariance, case sensitivity, deep-merge determinism, avalanche on content change, parameter-blindness
- [ ] Snapshot tests via insta lock every `--json` response shape; intentional schema changes require explicit snapshot review
- [ ] Cross-machine fixture test passes on Linux x86_64 + Linux aarch64 + macOS CI runners (Windows still deferred to Phase 7)
- [ ] Functional spec §7.5 scripted-comparison loop runs end-to-end against three fixture namespaces

**PRD coverage:** R-51, R-76, R-77, R-80, R-81, R-84, R-85, R-86, R-87 (versioning hook present, only `0x01` implemented).

**Functional spec coverage:** §5.6 diff (both flavors), §7.1 structured output, §7.2 project-scoped activation in scripts, §7.3 resolved-hash usage, §7.5 scripted comparison.

---

## Phase 6 — Shell integration, remotes, sync

**Deliverable:** `cd` between projects auto-activates the right namespace via a shell hook. Users can configure git remotes, install namespaces from a remote, push/pull the registry, and promote project-local edits back into their source namespace.

**Why this phase exists:** This is the "Tuesday workflow" phase. Up through Phase 5, every activation is manual. Phase 6 makes aenv feel like direnv: transparent, fast, ambient.

**Files to create:**
- `aenv-core/src/sync.rs` — registry push/pull via `git`
- `aenv-core/src/install.rs` — fetch named namespaces from configured remotes
- `aenv-core/src/promote.rs` — copy project edits back to namespace, re-symlink
- `aenv-cli/src/cmd/{init_shell,install,sync,promote,remote_add}.rs`
- `aenv-cli/src/cmd/activate_if_needed.rs` — fast-path used by shell hook
- `aenv-cli/src/shell/{bash,zsh,fish}.sh` — embedded via `include_str!`
- `benches/hook_latency.rs` — criterion benchmark for hook common case
- `tests/sync.rs`, `tests/install.rs`, `tests/hook_idempotence.rs`

**Phase completion criteria:**
- [ ] `aenv init-shell <bash|zsh|fish>` prints a hook script suitable for sourcing
- [ ] Shell hook calls `aenv activate-if-needed`; common-case (no change needed) runs in < 10 ms (criterion-benchmarked, regression > 2× fails CI)
- [ ] `aenv activate-if-needed` reads only `.aenv` pin + `.aenv/state.json` on the no-change path; performs no `extends` resolution unless namespace identity changed
- [ ] Cross-project `cd` (`A → B`, both pinned, different namespaces) deactivates A and activates B as one atomic transition
- [ ] `cd` out of all `.aenv`-covered directories deactivates cleanly
- [ ] Nested-project resolution: nearest-ancestor `.aenv` wins
- [ ] `aenv remote add <name> <url>` writes to `~/.aenv/config.toml`; `aenv remote list` enumerates
- [ ] `aenv install` fetches missing namespaces named in `.aenv`; resolves dependencies (extends, imported skills)
- [ ] `aenv sync` push/pull the registry against configured remotes; reports per-namespace changes
- [ ] `aenv promote <path>` copies project-local edits back to source namespace; re-establishes symlink
- [ ] Git availability probe + error mapping → exit 14 for any remote failure
- [ ] Manual smoke test: source the hook in zsh, `cd` between three pinned projects, observe correct activation/deactivation
- [ ] Integration tests cover all remote ops via a local git server fixture (file-backed bare repo in tempdir)

**PRD coverage:** R-35, R-36, R-39, R-40, R-41, R-42, R-55, R-56, R-57, R-58, R-59, R-64, R-65.

**Functional spec coverage:** §5.4 auto-activation, §5.7 forking project-local file, §5.8 teammate-onboarding flow.

---

## Phase 7 — Polish, cross-platform, release

**Deliverable:** Cross-platform CI green (Linux + macOS + Windows). Windows symlink fallback to copy-mode tested. Hash-algorithm versioning dual-emit infrastructure in place. Documentation refreshed. v0.1.0 tagged.

**Why this phase exists:** Phases 0–6 ship features; Phase 7 hardens the public surface. Some hardening can't happen earlier (e.g., Windows CI without a Windows symlink fallback would just spam red).

**Files to create / modify:**
- Add Windows runners to `.github/workflows/ci.yml`
- `aenv-core/src/activate.rs` — Windows symlink-fail fallback to copy with `copy-managed` state record
- `aenv-core/src/hash.rs` — extend with v2-ready dual-emit infrastructure (only v1 active)
- `aenv-cli/src/main.rs` — `--verbose` flag, `tracing` integration, exit-code documentation in `--help`
- `pm_docs/aenv-prd.md` → v0.4 (apply any deltas discovered in implementation)
- `pm_docs/aenv-functional-spec.md` → v0.4
- `pm_docs/aenv-engineering.md` → v0.3
- `README.md` — public-facing usage doc with quickstart
- `CHANGELOG.md` — initial entry

**Phase completion criteria:**
- [ ] Windows CI green on the full test matrix; symlink failures fall back to copy and are flagged in `state.json` as `copy-managed`
- [ ] Dual-emit hash infrastructure exists (an interface where adding a v2 algorithm and emitting both is mechanical, not architectural)
- [ ] `--verbose` flag toggles DEBUG logs; hook is silent on the happy path
- [ ] `aenv --help` and `aenv <command> --help` document all exit codes (R-83)
- [ ] All three pm_docs versions updated to reflect any spec deltas surfaced during implementation
- [ ] README.md covers install, quickstart (three-namespace setup), and a link to pm_docs/
- [ ] CHANGELOG.md initial entry documents the v0.1.0 release
- [ ] `git tag v0.1.0` lands on green CI
- [ ] Fresh-machine install test: a clean Linux/macOS/Windows VM can install aenv, run through functional spec §5.1–§5.4, and succeed

**PRD coverage:** R-49 Windows copy-mode, R-83 documentation completeness.

---

## Requirement → phase mapping

This is the audit table. Every R-number from the PRD appears exactly once on the left; the right column names the phase that delivers it. Where a requirement is split across phases (e.g. `aenv status` text in Phase 1, `--json` flavor in Phase 5), both phases are listed with the slice each owns. Use this to verify nothing in the PRD has slipped through the planning.

| Req | Phase | Note |
|---|---|---|
| R-1 registry directory | 1 | |
| R-2 `aenv create` | 1 | |
| R-3 `aenv list` (text) | 1 | `--json` flavor in Phase 5 |
| R-4 `aenv delete` | 1 | |
| R-5 reject duplicate name | 1 | |
| R-6 manifest required | 2 | |
| R-7 extends-chain resolution + default merge strategies | 2 | |
| R-8 `<!-- aenv:replace -->` directive | 2 | |
| R-9 qualified identity for every artifact | 2 | |
| R-10 shadow tracking | 2 | |
| R-11 short-vs-qualified separator `::` | 2 | |
| R-12 cycle detection | 2 | |
| R-13 missing-adapter refusal | 1 | thrown during Phase 1's adapter loading; surface in Phase 2 once `extends` exists |
| R-14 authored vs imported skill mode | 4 | |
| R-15 authored skills under namespace tree | 4 | |
| R-16 imported skill sources (local / git / registry) | 4 | Registry source-kind stubbed; PRD open question |
| R-17 activation resolves imported skills | 4 | |
| R-18 imported-skill pinning | 4 | |
| R-19 `aenv skill new` | 4 | |
| R-20 `aenv skill import` | 4 | |
| R-21 `aenv skill list` (text) | 4 | `--json` flavor in Phase 5 |
| R-22 unreachable import → skip vs `required = true` fail | 4 | |
| R-23 authored + imported coexist | 4 | |
| R-24 instructions-file size tracking | 3 | |
| R-25 per-adapter soft limits (5,000 / 6,000) | 3 | |
| R-26 `instructions_budget` parameter override | 3 | |
| R-27 `aenv doctor` reports size violations | 3 | |
| R-28 warnings never block activation, never truncate | 3 | |
| R-29 adapter plugin model | 1 (start) / 2 (extended) | Phase 1 loads one adapter; Phase 2 adds the rest |
| R-30 seven built-in adapters | 1 (claude-code) / 2 (cursor, aider, cline, continue, windsurf, mcp) | |
| R-31 `aenv adapter add` | 1 | |
| R-32 `aenv adapter list` (text) | 1 | `--json` flavor in Phase 5 |
| R-33 `.aenv` pin file format | 1 | |
| R-34 `aenv use` writes pin | 1 | |
| R-35 `aenv install` fetches | 6 | |
| R-36 `.aenv` names missing-and-unfetchable namespace → warn + decline | 6 | |
| R-37 `aenv activate` | 1 | |
| R-38 `aenv deactivate` | 1 | |
| R-39 shell hook scripts (bash/zsh/fish) | 6 | |
| R-40 auto-activate on `cd` in | 6 | |
| R-41 auto-deactivate on `cd` out | 6 | |
| R-42 atomic cross-project transition | 6 | |
| R-43 `.aenv/state.json` on activation | 1 | extended in Phase 2 (qualified provenance), Phase 3 (parameters), Phase 4 (skill provenance) |
| R-44 symlink for new files | 1 | |
| R-45 backup displaced files | 1 | |
| R-46 byte-identical → leave in place + mark managed | 1 | |
| R-47 merged file → write regular file, record contributors | 2 | depends on merge implementations |
| R-48 deactivate removes only what aenv materialized | 1 | |
| R-49 Windows copy-mode fallback | 7 | |
| R-50 `aenv status` | 1 (single-ns text) / 2 (chain) / 5 (`--json` + parameters + hash) | |
| R-51 `aenv diff` (project drift) | 5 | structural-diff flavor (`aenv diff <a> <b>`) per functional spec §5.6 also lands here |
| R-52 `aenv which` with shadow chain | 2 | |
| R-53 `aenv fork` in-project | 2 | |
| R-54 `aenv fork <name>` to new namespace | 2 | |
| R-55 git remotes configurable | 6 | |
| R-56 `aenv sync` push/pull | 6 | |
| R-57 `aenv install` precedence + report source | 6 | |
| R-58 `aenv remote add` | 6 | |
| R-59 `aenv promote` | 6 | |
| R-60 never modify outside adapter-declared paths | 1 | invariant, enforced from Phase 1 onward |
| R-61 never delete backups except via explicit cleanup | 1 | |
| R-62 `aenv restore` | 1 | |
| R-63 rollback on partial activation failure | 1 | |
| R-64 `aenv init-shell <bash|zsh|fish>` | 6 | |
| R-65 hook is the only auto-activate trigger | 6 | |
| R-66 `[parameters]` table | 3 | |
| R-67 parameter inheritance (last-wins) | 3 | |
| R-68 adapter declares consumed parameters | 3 | |
| R-69 `aenv get` | 3 | |
| R-70 `aenv set` | 3 | |
| R-71 type-incompatible value → ManifestInvalid | 3 | |
| R-72 `[policies]` table | 3 | |
| R-73 `aenv doctor` evaluates own + inherited | 3 | |
| R-74 `enforce = true` blocks activation | 3 | |
| R-75 child cannot weaken parent enforce | 3 | |
| R-76 `--json` on every read command | 5 | |
| R-77 qualified names in all `--json` | 5 | |
| R-78 `--project <path>` accepted everywhere | 1 | invariant, plumbed from Phase 1 |
| R-79 `aenv activate <name> --project <path>` | 1 | |
| R-80 resolved-namespace hash in `status --json` and `list --json` | 5 | |
| R-81 hash changes iff resolved content changes | 5 | property-tested |
| R-82 distinct non-zero exit codes | 1 (codes 10, 11, 12, 13, 20) / 3 (16, 17) / 4 (git-on-PATH starts surfacing 14) / 6 (14 complete) | |
| R-83 exit codes documented in `--help` | 1 (initial) / 7 (full) | |
| R-84 hash canonicalization algorithm | 5 | |
| R-85 hash exposed as `sha256-v1:<hex>` | 5 | |
| R-86 hash excludes manifest formatting / timestamps / etc. | 5 | property-tested |
| R-87 hash-algorithm versioning + dual-emit window | 7 | infrastructure only — v2 not implemented |

## Section → phase mapping

The same audit at PRD-section granularity. Useful when stepping back to confirm whole sections are covered, not just individual requirements.

| PRD §  | Title                                       | Phases that cover it |
|---|---|---|
| 5.1    | Namespace lifecycle                         | Phase 1 |
| 5.2    | Manifest, composition, namespace identity   | Phase 2 |
| 5.3    | Skill content model                         | Phase 4 |
| 5.4    | Instructions-file size limits               | Phase 3 |
| 5.5    | Adapters                                    | Phase 1 (claude-code + adapter commands) / Phase 2 (remaining built-ins) |
| 5.6    | Project pinning                             | Phase 1 (pin file + `use`) / Phase 6 (`install`) |
| 5.7    | Activation and auto-activation              | Phase 1 (manual `activate`/`deactivate` + state) / Phase 6 (shell-hook auto-activation) |
| 5.8    | File materialization and conflicts          | Phase 1 (symlink, backup, no-op, deactivate-restore) / Phase 2 (merged-file materialization) / Phase 7 (Windows copy fallback) |
| 5.9    | Status and introspection                    | Phase 1 (`status` text, single namespace) / Phase 2 (`which`, chain provenance) / Phase 5 (`diff`, `--json` everywhere) |
| 5.10   | Forking and divergence                      | Phase 2 |
| 5.11   | Sync and sharing                            | Phase 6 |
| 5.12   | Safety and reversibility                    | Phase 1 |
| 5.13   | Shell integration                           | Phase 6 |
| 5.14   | Namespace parameters                        | Phase 3 |
| 5.15   | Namespace-scoped policies                   | Phase 3 |
| 5.16   | Scriptability and machine interfaces        | Phase 1 (`--project`, initial exit codes) / Phase 3 (codes 16, 17) / Phase 5 (`--json`, qualified-names everywhere, hash exposure) / Phase 6 (code 14 complete) / Phase 7 (`--help` exit-code docs) |
| 5.17   | Resolved-namespace hash specification       | Phase 5 (R-84/R-85/R-86 — the algorithm itself) / Phase 7 (R-87 dual-emit infrastructure) |

Engineering-doc sections referenced in the roadmap: §3 error strategy (Phase 0) · §4 adapter loading (Phase 1) · §5 Filesystem trait (Phase 0) · §6 path handling (Phase 1) · §7 atomicity (Phase 1) · §7.5 namespace identity (Phase 2) · §8 testing (every phase) · §9 performance budget (Phase 6) · §10 git shell-out (Phase 4 first surfaces, Phase 6 completes) · §11 versioning (Phase 7).

---

## Cross-cutting practices (every phase)

- **TDD strict:** every behavior gets a failing test before the implementation, per `superpowers:test-driven-development`.
- **Frequent commits:** one logical change per commit; commit when its tests pass.
- **No new public surface without a snapshot test** once we're past Phase 5.
- **Update pm_docs/ deltas inline:** if implementation reveals a spec gap or surfaces a needed change, propose the doc patch alongside the code patch in the same PR.
- **Lessons file:** capture user corrections in `tasks/lessons.md` per CLAUDE.md global instructions.
- **Verification before completion:** every phase-completion checkbox is gated on the verification commands in the integration tests, not just on the implementer's belief that it works (`superpowers:verification-before-completion`).

---

## What is NOT in this roadmap

- GUI / dashboards / editor plugins (PRD §8 — explicit out-of-scope).
- Behavioral evaluation of agents under different namespaces (separate downstream project that consumes our JSON surface).
- Encryption of namespace contents (PRD §8).
- Per-file ACLs or fine-grained namespace permissions (PRD §8).
- Runtime namespace isolation — multiple namespaces active simultaneously, agent-side namespace awareness (PRD §3 non-goal).
- Cross-namespace artifact references (PRD §8 — only `extends` is the composition primitive).
- Skill registry as a source type for imported skills (R-16 third option) — stubbed but not implemented; PRD open question.
- Plugin / WASM adapters (engineering §4 option 3 — held in reserve behind a feature flag).
- Telemetry of any kind (engineering §12).

---

## Open questions tagged to phase boundaries

- **Phase 0 → 1:** finalize the `Filesystem` trait surface (~12 methods) before Phase 1 writes its first integration test. Adding a method later is cheap; removing one is not.
- **Phase 2 → 3:** confirm the parameter-projection mechanism — pure declarative TOML in the adapter file, or a small Rust trait per adapter? Engineering §4 leaves this slightly open. Default to declarative; bump to trait only if a built-in adapter needs branching logic.
- **Phase 4 → 5:** confirm cache layout for git-imported skills. Default `~/.aenv/cache/skills/<source-hash>/<ref>/` unless a better scheme surfaces during implementation. The hash uses what scheme — SHA-256 of the source URL? Lock that before Phase 4 ships.
- **Phase 6 → 7:** decide on the v1.0 milestone gating. Phase 7 produces v0.1.0; the move to v1.0.0 should follow a dogfooding period (one or two contributors using it daily for a month). Out of scope for this roadmap to commit a date.

---

## Next step

This roadmap defines milestones at the phase level. The next planning step is a detailed bite-sized plan for **Phase 0** — every step expanded to file paths, test code, and commit boundaries per `superpowers:writing-plans`. That detailed plan will land at `tasks/2026-05-20-phase-0-skeleton.md` once we agree the roadmap shape is right.
