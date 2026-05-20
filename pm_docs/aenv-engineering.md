# Engineering Doc: aenv

**Companion to:** PRD v0.3 (namespaces), Functional Spec v0.3
**Status:** Draft
**Last updated:** 2026-05-19

This document covers implementation-level decisions that the PRD and functional spec deliberately omit. It is owned by engineering and may evolve independently as long as it does not violate the public contracts defined in the PRD.

---

## 1. Implementation language: Rust

`aenv` is implemented in Rust. The relevant trade-offs:

- **Single static binary.** Users install `aenv` once and it runs on any machine of the same architecture with no runtime dependency. This matters for a tool that's invoked on every `cd` — interpreter startup costs and runtime version pinning would degrade the user experience.
- **Predictable performance.** The shell hook runs synchronously in the user's prompt. Sub-10ms invocation is achievable in Rust without effort and difficult in interpreted languages.
- **Stronger guarantees around the contracts that matter.** Exit codes, JSON output schemas, and the resolved-env hash are public contracts. Rust's type system and `serde` ecosystem make schema drift surfacing at compile time.

The cost is development velocity. We accept this because `aenv`'s feature surface is small and well-bounded; it's a tool that should be polished rather than prolific.

## 2. Crate selection

| Concern | Crate | Notes |
|---|---|---|
| CLI parsing | `clap` v4 (derive) | Subcommand structure maps cleanly to derive macros |
| Manifests (TOML) | `toml` + `serde` | Manifests are pure data |
| JSON output / merging | `serde_json` | Also used for RFC 8785 canonicalization |
| YAML merging | `serde_yaml` | Converted to canonical JSON before hashing |
| Hashing | `sha2` | SHA-256 only; no need for the wider `digest` ecosystem |
| Temp directories | `tempfile` | Backbone of integration testing |
| Cross-platform paths | `directories` | `~/.aenv` resolution on macOS, Linux, Windows |
| Git operations | `std::process::Command` (shell out to `git`) | Assumes `git` is on `PATH`; see §10 |
| Error definitions | `thiserror` | For the `AenvError` enum below |
| Property testing | `proptest` | For hash invariants |
| Snapshot testing | `insta` | For JSON output schemas |
| RFC 8785 JCS | (custom implementation) | No mature crate at time of writing; ~150 lines |

## 3. Error strategy

All fallible operations return `Result<T, AenvError>`. `AenvError` is a `thiserror`-derived enum where each variant corresponds to one documented exit code from PRD R-67. The CLI layer is the only place that converts `AenvError` to an exit code.

```rust
#[derive(Debug, thiserror::Error)]
pub enum AenvError {
    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),              // exit 10
    #[error("adapter '{0}' is not installed")]
    AdapterMissing(String),                 // exit 11
    #[error("manifest invalid: {0}")]
    ManifestInvalid(String),                // exit 12
    #[error("activation conflict: {0}")]
    ActivationConflict(String),             // exit 13
    #[error("remote unreachable: {0}")]
    RemoteUnreachable(String),              // exit 14
    #[error("cycle in extends chain: {0}")]
    ExtendsCycle(String),                   // exit 15
    #[error("parameter '{0}' is undefined in the resolution chain")]
    ParameterUndefined(String),             // exit 16
    #[error("policy violation: {0}")]
    PolicyViolation(String),                // exit 17
    #[error("project not pinned")]
    ProjectNotPinned,                       // exit 20
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),             // exit 1
    // ...
}

impl AenvError {
    pub fn exit_code(&self) -> i32 { /* match self { ... } */ }
}
```

`anyhow` is used **only** at command-handler boundaries when chaining context for human-readable error messages. It is never returned across the library API.

Rationale: PRD R-67/R-68 make exit codes a public contract. Boxing errors via `Box<dyn Error>` would lose the discriminant; bare `anyhow::Error` loses it too. The enum is the cost of keeping the contract honest. The `NamespaceNotFound` variant keeps the `Namespace` prefix even though internal helpers may still use `env` in legacy field names — the error message is what users see, and consistency in the public surface matters more than internal naming purity.

## 4. Adapter loading

Adapters are TOML files loaded at runtime from `~/.aenv/adapters/`. They are shared across namespaces — adapter definitions live outside the namespace tree because adapters describe *tools*, while namespaces describe *configurations of those tools*. They are pure data: paths the adapter manages and merge strategies per path. No code, no plugins, no dynamic loading.

```toml
# ~/.aenv/adapters/claude.toml
name = "claude"
files = ["CLAUDE.md", ".claude/"]

[merge_strategies]
".mcp.json" = "json-deep"
```

This was a choice among three options:

1. **Compiled-in adapters only.** Simplest. Adapters require a PR to add. Rejected because the PRD explicitly calls for `aenv adapter add <path>`.
2. **TOML-declared adapters.** Selected. Adapter logic so far is purely declarative (paths + merge strategies); adding code-level customization isn't needed for v1.
3. **Dynamic libraries or WASM plugins.** Powerful but adds significant complexity (sandbox, ABI, distribution). Held in reserve. If a future adapter needs custom logic — say, a tool that requires templating configs based on the current git branch — we'll add a plugin trait behind a feature flag without breaking the TOML path.

Built-in adapters (Claude, Cursor, Aider, Cline, Continue, MCP) ship as embedded TOML strings via `include_str!` and are written to disk on first run. Users can override them by writing a same-named adapter file; the user file wins.

## 5. Filesystem abstraction for testability

A `Filesystem` trait isolates all I/O. Production code uses `RealFilesystem`; tests use a programmable mock.

```rust
pub trait Filesystem {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn write(&mut self, path: &Path, contents: &[u8]) -> io::Result<()>;
    fn symlink(&mut self, src: &Path, dst: &Path) -> io::Result<()>;
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&mut self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&mut self, path: &Path) -> io::Result<()>;
    fn create_dir_all(&mut self, path: &Path) -> io::Result<()>;
    fn metadata(&self, path: &Path) -> io::Result<Metadata>;
    fn read_link(&self, path: &Path) -> io::Result<PathBuf>;
    fn exists(&self, path: &Path) -> bool;
    // ~12 methods total
}
```

Keep this surface narrow. Mocking `std::fs` wholesale is a tar pit; mocking the dozen operations `aenv` actually performs is tractable.

## 6. Path handling

All paths below the CLI layer are absolute. The CLI layer:

1. Resolves `AENV_HOME` (default `~/.aenv`) from environment variables. The `AENV_HOME` name is retained for backward compatibility; conceptually it points at the namespace registry.
2. Resolves the project root either from `--project <path>` or by walking up from the current directory looking for `.aenv`.
3. Passes absolute paths into the library.

The library never reads `std::env::current_dir()` or environment variables. This makes R-49 (`--project` flag) almost free — the plumbing already exists — and prevents an entire class of "works in tests but not in shell hooks" bugs.

## 7. Atomicity and the rename pitfall

PRD R-45 requires transactional rollback on activation failure. The implementation relies on `std::fs::rename` being atomic, which is true on Unix **only when source and destination are on the same filesystem**.

The backup directory is therefore always inside the project (`.aenv/backup/`), not in a system temp directory. If a project happens to live on a filesystem boundary (e.g. project root on one mount, `.aenv/` symlinked elsewhere), rename silently degrades to copy+delete and atomicity is lost.

**Mitigation:** at the start of activation, perform a probe rename in `.aenv/`. If it succeeds, proceed. If it crosses a filesystem boundary, abort with `ActivationConflict` and surface a clear message. This is cheap and protects R-45.

## 7.5 Namespace identity in the internal model

Namespace identity is the substrate that makes `aenv which`, qualified-name JSON output, shadow chains, and the `aenv diff <a> <b>` command possible. The internal representation is intentionally explicit about it.

Every resolved artifact is keyed by a `(NamespaceId, ShortName)` tuple:

```rust
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct NamespaceId(String);   // e.g. "detailed-execution"

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ShortName(String);     // e.g. "write-tests"

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct QualifiedName {
    pub namespace: NamespaceId,
    pub short: ShortName,
}

impl fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // The display form uses `::` as separator.
        // This is the wire format and may not change without a major version.
        write!(f, "{}::{}", self.namespace.0, self.short.0)
    }
}
```

**Separator choice.** `::` is used for namespace qualification (`detailed-execution::write-tests`). `.` is used for parameter access (`detailed-execution.default_model`). Keeping these visually distinct prevents confusion: `::` says "this is an artifact owned by this namespace"; `.` says "this is a parameter declared by this namespace." The separators are part of the public CLI surface and the JSON schema; changing them is a major-version break.

**Resolution output.** Resolving a namespace produces a `ResolvedNamespace` value containing the full materialization plan plus shadow tracking:

```rust
pub struct ResolvedNamespace {
    pub chain: Vec<NamespaceId>,            // base → ... → leaf
    pub artifacts: Vec<ResolvedArtifact>,
    pub parameters: BTreeMap<String, ResolvedParameter>,
    pub policies: BTreeMap<String, ResolvedPolicy>,
}

pub struct ResolvedArtifact {
    pub qualified_name: QualifiedName,
    pub materialized_path: PathBuf,         // short name, on disk
    pub source_path: PathBuf,               // in the namespace's directory
    pub strategy: MaterializeStrategy,      // Symlink | Copy | SectionMerge | DeepMerge
    pub shadows: Vec<QualifiedName>,        // empty when no shadowing
    pub contributors: Vec<QualifiedName>,   // for merged files
}
```

**Shadowing.** When `detailed-execution::write-tests` and `base::write-tests` both want to materialize at `.claude/skills/write-tests/SKILL.md`, the resolver records:
- `qualified_name`: `detailed-execution::write-tests` (the winner)
- `shadows`: `[base::write-tests]` (the loser, preserved by identity)

This is the data backing `aenv which` and JSON `shadows` arrays. The shadowed artifact's content is *not* discarded from `aenv`'s view of the namespace — the user can inspect it by looking at the parent namespace's directory directly, and `aenv diff base detailed-execution` can compute the structural difference.

**Materialization is identity-erasing.** When `aenv` writes files to disk, it writes short names: `.claude/skills/write-tests/SKILL.md`, not `.claude/skills/detailed-execution::write-tests/SKILL.md`. Target tools have no namespace awareness; qualified names exist only in `aenv`'s internal state, the activation state file (`.aenv/state.json`), and machine output. This is a deliberate choice: namespaces are a *build-time* organizational concept, not a runtime concept the agent sees.

**Hash neutrality.** The resolved-namespace hash (PRD §5.15) is computed over the materialized file contents only. It does NOT incorporate qualified names, shadow chains, parameters, or policies. Two resolution chains that produce byte-identical materialized output share a hash — even if one used `base::write-tests` and the other used `detailed-execution::write-tests` with identical content. Downstream consumers that need to attribute behavior to a specific namespace (rather than to its materialized content) must record the namespace name and parameters separately, alongside the hash.

## 8. Testing strategy

### 8.1 Integration tests (happy paths)

Each test creates two temp directories — one for `AENV_HOME`, one for the project — and runs real commands against real files. Tests assert by reading the filesystem back.

Structure:

```
tests/
├── common/
│   └── mod.rs            # TestEnv harness: temp dirs, helper builders
├── lifecycle.rs          # create, list, delete, edit envs
├── activation.rs         # activate, deactivate, switch, auto-via-hook simulation
├── composition.rs        # extends chains, merge strategies, cycle detection
├── conflicts.rs          # backup-on-displacement, restore
├── scriptability.rs      # --json schemas, --project flag, exit codes
└── hash.rs               # cross-machine hash agreement (parameterized)
```

Slow — each test pays for filesystem ops — but truthful. Run on every PR.

### 8.2 Mocked tests (edge cases)

For things integration tests can't easily induce: symlink failure on Windows, disk-full mid-activation, file appearing between check and write, permission errors on the backup directory.

These live under `src/` next to the modules they test, using the `Filesystem` trait's mock implementation.

### 8.3 Property tests for the hash and namespace identity

The hash and namespace identity model are both public contracts. These properties are verified with `proptest`:

**Hash properties:**
- **Order independence in the manifest.** Permuting the order of `files` entries in a manifest produces the same hash.
- **Whitespace invariance.** Reformatting `aenv.toml` (extra blank lines, comment changes) produces the same hash.
- **Case sensitivity in paths.** Two namespaces differing only in path case produce different hashes.
- **Deep-merge determinism.** Two `extends` chains that produce the same merged output produce the same hash.
- **Avalanche on content change.** Any single-byte change in any resolved file changes the hash.
- **Hash is parameter-blind.** Two namespaces with identical materialized files but different parameters produce the same hash. (This is a deliberate property: parameters are metadata, not content. Downstream tools that care about parameters must record them separately.)

**Namespace identity properties:**
- **Qualified-name uniqueness within a namespace.** Within a single namespace, every artifact has a unique short name; the resolver rejects manifests that violate this.
- **Shadow preservation.** When two namespaces in the chain provide artifacts at the same materialized path, the resolver always records the loser in the winner's `shadows` field. No silent drops.
- **Shadow chain ordering.** When three namespaces in a chain all provide an artifact at the same path, the leaf wins and the shadow list contains the other two in `extends` chain order (oldest first).
- **Materialization uses short names.** No materialized path on disk contains `::`. This is an invariant tested by walking the project tree after activation.

```rust
proptest! {
    #[test]
    fn manifest_order_doesnt_affect_hash(files in any_file_list()) {
        let h1 = hash_env(build_env(files.clone()));
        let h2 = hash_env(build_env(shuffle(files)));
        prop_assert_eq!(h1, h2);
    }
}
```

### 8.4 Snapshot tests for JSON output

`insta` locks the shape of every `--json` response. Any change to the JSON schema is visible in code review as a snapshot diff.

```rust
#[test]
fn status_json_schema() {
    let output = run(&["status", "--json", "--project", &project]);
    insta::assert_json_snapshot!(output);
}
```

This is the contract test between `aenv` and any downstream consumer (including the future eval project). Schema changes that aren't intentional get caught here.

### 8.5 Cross-machine hash test

The cross-machine guarantee (R-51) is hard to test directly. Instead:

- Check fixture envs into the test suite as serialized byte streams along with their expected hashes.
- Verify that loading the fixture and recomputing the hash yields the expected value.
- Run this test on all CI platforms (Linux x86_64, Linux aarch64, macOS, Windows).

Any platform-dependent behavior (line endings, path separators, encoding) surfaces as a hash mismatch.

## 9. Performance budget

The shell hook runs synchronously on every `cd`. Target: **under 10ms** for the common case (no namespace change needed, just confirming current state).

This implies:

- The state check should require reading at most one or two small files (`.aenv` pin, `.aenv/state.json`).
- Resolved-env hashing happens only on activation, not on every hook invocation.
- No git operations in the hook path. `aenv install` and `aenv sync` are explicit user actions.

Benchmark these in CI with `criterion`. Regressions over 2× the baseline fail the build.

## 10. External dependency: `git`

`aenv` shells out to the system `git` binary for all sync operations (`aenv install`, `aenv sync`, `aenv remote add` validation). We do not link a Git library into the binary.

**Why shell out.** Anyone using `aenv` is on a developer machine, and developer machines have `git`. Shelling out gives us a smaller binary, faster compile times, no C toolchain requirement for contributors, free upgrades when users update their `git`, and inheritance of the user's existing Git configuration — credentials, SSH agent, GPG signing, HTTPS proxies — without having to re-implement any of it. It also removes libgit2 from our security review surface.

**The contract.** `aenv` assumes `git` is available on `PATH`. The set of operations we invoke is small: `clone`, `pull`, `push`, `fetch`, `ls-remote`. We require Git 2.20 or later (released 2018), which is older than any actively supported distribution.

**Detection and failure mode.** At the start of any command that touches a remote, `aenv` probes for `git` via `Command::new("git").arg("--version")`. If the probe fails:

```
error: 'git' not found on PATH
       aenv requires git for remote operations (sync, install).
       Install git from https://git-scm.com/downloads, or run aenv
       in a project that doesn't depend on remote namespaces.
exit code: 14  (remote unreachable)
```

The probe is skipped for commands that don't touch remotes — `activate`, `status`, `use`, etc. all work without `git` on the system. This matters: a user with a fully local registry should never see a `git`-related error.

**Error mapping.** Git command failures are captured as exit code + stderr and mapped to `AenvError::RemoteUnreachable` with the stderr included in the user-facing message. We do not parse stderr to extract structured information — Git's error messages are not a stable interface.

**What we don't do.** No interactive prompts (Git's own prompts for credentials are fine and pass through; `aenv` adds none of its own). No long-running Git operations in the shell hook path — R-47 already implies this, but it's worth saying explicitly that the hook never invokes `git`.

## 11. Versioning and compatibility

- **Semver.** Pre-1.0 means anything can change. Post-1.0, breaking the JSON schema, exit codes, hash algorithm, namespace separator (`::`), parameter separator (`.`), or `.aenv` file format is a major-version bump.
- **Hash algorithm versioning.** Per PRD R-72, algorithm changes carry a version byte. We commit to emitting both old and new hashes for at least one major release after introducing a new algorithm.
- **Namespace and parameter separators.** `::` and `.` are part of the public CLI and JSON contract. Changing either requires a major version bump and a deprecation window with both forms accepted.
- **State file forward-compatibility.** `.aenv/state.json` includes a `schema_version` field. Older `aenv` reading a newer state file refuses to activate and prints an upgrade hint rather than silently mishandling fields it doesn't understand.
- **Git version.** Minimum supported `git` is 2.20. If we ever need a feature from a later version, that's a documented bump with a version probe at startup.

## 12. Open implementation questions

- **Async or sync?** Filesystem operations are blocking. We don't need async for v1 — the only concurrency is parallel hashing of large envs, which can use `rayon` if profiling shows it matters. Default to sync; add parallelism only when measured.
- **Logging.** `tracing` with structured fields. The shell hook should be silent on the happy path; everything else uses INFO and above. A `--verbose` flag bumps it to DEBUG.
- **Telemetry.** None. `aenv` doesn't phone home. If we ever add opt-in usage metrics, that's a separate proposal with its own privacy review.

## 13. What's deliberately not decided yet

- The exact JSON schema for `aenv diff --json`. Specify it when we implement diff.
- Whether `aenv promote` should require confirmation. Probably yes, but UX-test before deciding.
- How `aenv install` handles version pinning of namespaces (commit SHA vs. branch). Defer to the sync redesign once we have multiple users hitting it.
