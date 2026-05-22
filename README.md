# aenv — Virtual environments for AI coding harness configs

`aenv` is a Rust CLI for managing named, composable, version-controlled bundles of AI-coding-agent configuration (`CLAUDE.md`, `.cursorrules`, `.mcp.json`, skills, agents, slash commands, MCP entries). Think Python's `venv`, but for the rules and configurations that shape how AI coding agents behave.

> **Status:** Active development. Phase 3 (parameters & policies) is the most recent milestone, tagged [`phase-3-complete`](../../tree/phase-3-complete). The roadmap is in [`tasks/todo.md`](./tasks/todo.md).

## What works today

After `phase-3-complete`, `aenv` can:

- **Create and compose namespaces.** A namespace bundles `CLAUDE.md`, `.cursorrules`, skills, agents, settings — anything an AI coding harness reads — and can `extends` another namespace. Composition produces section-merged Markdown, deep-merged JSON / YAML / TOML, and qualified-name provenance for every artifact. Cycles are caught (exit 15).
- **Pin and activate projects.** `aenv use <name>` writes a `.aenv` pin file; `aenv activate` materializes the resolved namespace as symlinks (or merged files where strategy demands) and records every move in `.aenv-state/state.json`. `aenv deactivate` puts the project back exactly as it was, restoring any files it displaced.
- **Inspect provenance.** `aenv status` shows the resolution chain, every managed file with its qualified source, the shadow chain, effective parameters, and active policies. `aenv which <path>` answers "where did this file come from?".
- **Declare typed parameters and policies.** Manifests carry `[parameters]` (string / int / bool / list-of-string) that inherit last-wins across the extends chain, and `[policies]` (advisory by default, or `enforce = true`) that inherit with R-75 enforce-protection — a child can tighten but not weaken a parent's enforced policy.
- **Run a doctor check.** `aenv doctor [<ns>]` evaluates four built-in policy evaluators (`instructions_max_chars`, `skill_requires_description`, `mcp_requires_command_or_url`, `forbid_paths`) against the resolved namespace and prints per-policy outcomes. Enforced violations also block `aenv activate` with exit 17 — *before* any file is touched.
- **Read and write parameters from the CLI.** `aenv get <ns>.<param>` or `aenv get .<param>` (active project) shows the effective value with provenance; `aenv set <ns>.<param> <value>` rewrites the named namespace's manifest, inferring the value type.
- **Fork to a private copy.** `aenv fork` detaches a whole project from its namespace (replacing symlinks with copies); `aenv fork <file>` detaches just one file; `aenv fork <name>` creates a new namespace populated from the current project state.

Ships with built-in adapters for **Claude Code, Cursor, Aider, Cline, Continue, Windsurf, and a generic MCP adapter** — all embedded in the binary, written to `~/.aenv/adapters/` on first run, and overridable by user edit.

## What's still in flight

The roadmap (see [`tasks/todo.md`](./tasks/todo.md)) has four phases left:

- **Phase 4** — Skills lifecycle: `aenv skill new`, `aenv skill import` (local + git), pinned vs floating refs.
- **Phase 5** — Resolved-namespace hash + `--json` on every read-oriented command + `aenv diff`. Designed for downstream eval tools.
- **Phase 6** — Shell integration (`cd`-based auto-activation), git remotes, `aenv install`, `aenv sync`, `aenv promote`.
- **Phase 7** — Windows symlink fallback, cross-platform CI, v0.1.0 release.

## Quick start

```bash
# Build (Rust 1.85+ stable)
cargo build --release --workspace
alias aenv=./target/release/aenv     # or copy to ~/.local/bin/aenv

# Create a namespace
aenv create base
$EDITOR ~/.aenv/envs/base/aenv.toml  # add [adapters], [parameters], [policies]
$EDITOR ~/.aenv/envs/base/CLAUDE.md  # author harness content

# Pin and activate in a project
cd ~/code/my-project
aenv use base
aenv activate
aenv status         # see what's active
aenv doctor base    # check policy compliance
```

Functional spec §2 sketches three example harnesses (`experiments`, `detailed-execution`, `analyst`) that illustrate the intended composition style.

## Reading order

- **[`pm_docs/aenv-prd.md`](./pm_docs/aenv-prd.md)** — Product requirements in EARS format. The public contract (87 requirements, R-1 through R-87).
- **[`pm_docs/aenv-functional-spec.md`](./pm_docs/aenv-functional-spec.md)** — How users interact with `aenv`. Three example harnesses, twelve user journeys, `doctor` / `diff` / scriptability examples.
- **[`pm_docs/aenv-engineering.md`](./pm_docs/aenv-engineering.md)** — Internal implementation decisions: Rust, crate selection, error / exit-code strategy, `Filesystem` trait, namespace identity model, hash specification.
- **[`tasks/todo.md`](./tasks/todo.md)** — Phase-by-phase implementation roadmap mapped back to PRD requirements.
- **[`tasks/2026-05-22-phase-3-parameters-policies.md`](./tasks/2026-05-22-phase-3-parameters-policies.md)** — Most recent implementation plan (20 tasks, bite-sized, with code and tests inline). Earlier phase plans live alongside it.

## Building & testing

```bash
cargo build --workspace
cargo test --workspace                            # ~330 tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Requires Rust stable 1.85 or later. No external runtime dependencies.

## Exit codes

`aenv` uses distinct non-zero exit codes for documented failure classes — useful for scripting. The full table lives in [`aenv-core/src/error.rs`](./crates/aenv-core/src/error.rs); the most common are:

| Code | Meaning |
|---|---|
| 1  | Generic I/O error |
| 10 | Namespace not found |
| 11 | Adapter not installed |
| 12 | Manifest invalid (type mismatch, malformed TOML, R-75 weakening) |
| 13 | Activation conflict |
| 14 | Remote unreachable *(Phase 6)* |
| 15 | Cycle in extends chain |
| 16 | Parameter undefined |
| 17 | Policy violation (`enforce = true`) |
| 20 | Project not pinned |

## License

Dual-licensed under MIT or Apache 2.0.
