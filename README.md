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

Ships with built-in adapters for **Claude Code, Cursor, Aider, Cline, Continue, Windsurf, Codex, and a generic MCP adapter** — all embedded in the binary, written to `~/.aenv/adapters/` on first run, and overridable by user edit. Also ships with three starter namespaces (`karpathy`, `cherny`, `blank`) written to `~/.aenv/envs/` on first run so you have something to switch between out of the box.

## What's still in flight

The roadmap (see [`tasks/todo.md`](./tasks/todo.md)) has four phases left:

- **Phase 4** — Skills lifecycle: `aenv skill new`, `aenv skill import` (local + git), pinned vs floating refs.
- **Phase 5** — Resolved-namespace hash + `--json` on every read-oriented command + `aenv diff`. Designed for downstream eval tools.
- **Phase 6** — Shell integration (`cd`-based auto-activation), git remotes, `aenv install`, `aenv sync`, `aenv promote`.
- **Phase 7** — Windows symlink fallback, cross-platform CI, v0.1.0 release.

## Installation

`aenv` ships as a single static binary. Two install paths:

- **Pre-built binary (Linux + macOS).** Recommended once a release has been cut — see [`INSTALL_FROM_BINARY.md`](./INSTALL_FROM_BINARY.md) for the download / checksum / install steps. Windows is not yet supported (Phase 7).
- **Build from source.** The path below. Always works, no release dependency, and required for Windows users until the Phase 7 symlink fallback lands.

### Prerequisites

- **Rust toolchain 1.85 or newer.** Install via [rustup](https://rustup.rs):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Git**, to clone the repository.
- **A POSIX filesystem with symlink support.** Linux and macOS are fully supported today. Windows works for read-only commands (`aenv list`, `aenv status`); `aenv activate` needs the symlink fallback landing in Phase 7.

### Build and install

```bash
git clone https://github.com/blevene/aenv
cd aenv
cargo install --path crates/aenv-cli --locked
```

`cargo install` compiles the `aenv-cli` package and drops the `aenv` binary into `~/.cargo/bin/`, which `rustup` already adds to your shell's `PATH`. If you didn't install Rust via `rustup`, ensure `~/.cargo/bin` is on `PATH`:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc   # or ~/.bashrc
```

### Verify

```bash
aenv --version            # → aenv 0.0.1
aenv list                 # → shows the karpathy + cherny starter namespaces
```

The first invocation populates the registry at `~/.aenv/`:

```
~/.aenv/
├── adapters/             # 8 built-in adapter manifests (claude-code, codex, cursor, …)
└── envs/
    ├── karpathy/         # starter namespace + CLAUDE.md
    └── cherny/           # starter namespace + CLAUDE.md
```

Override the registry root with `AENV_HOME=/some/path aenv …` — useful for keeping work / personal configs separate, or for testing changes against a clean state.

### Updating

```bash
cd path/to/aenv && git pull
cargo install --path crates/aenv-cli --locked --force
```

`--force` is what tells `cargo install` to overwrite the existing binary. Your `~/.aenv/` registry — including any namespaces you've edited or created — is untouched.

### Uninstalling

```bash
cargo uninstall aenv-cli   # removes the binary
rm -rf ~/.aenv             # optional: discard the registry and your namespaces
```

## Quick start

```bash
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

## Try the built-in namespaces

`aenv` ships with three starter namespaces — `karpathy` (surgical, "minimum code to solve the problem"), `cherny` (plan-first, subagent-heavy), and `blank` (placeholder scaffold designed for editing) — all wired up against the `claude-code` adapter and materialized to `~/.aenv/envs/` automatically on first run. Use them to see the activate / switch / restore loop without authoring anything yourself.

```bash
aenv list                 # karpathy and cherny show up out of the box

cd ~/code/my-project
aenv use karpathy         # write the .aenv pin
aenv activate             # materialize CLAUDE.md (Symlink into ~/.aenv/envs/karpathy)
head -3 CLAUDE.md         # → "## 1. Think Before Coding"

# Swap to the other style without leaving the project
aenv deactivate           # restore whatever CLAUDE.md was there before (or remove it)
aenv use cherny
aenv activate
head -3 CLAUDE.md         # → "## Workflow Orchestration"

aenv deactivate           # back to the original project state
aenv unpin                # also drop the .aenv pin file
```

The starter namespaces are regular namespaces — edit `~/.aenv/envs/karpathy/CLAUDE.md` to tailor it, or copy one as the starting point for your own (`aenv create mine --extends karpathy`). Once a file exists on disk `aenv` won't overwrite it on subsequent runs, so your edits stick.

## Creating your own namespace

There are four common starting points, in roughly increasing order of "I have something in mind."

### 1. Use `blank` as a scratchpad

The shipped `blank` namespace is a placeholder designed for editing. Activate it in a project, edit the materialized `CLAUDE.md` (which is a symlink pointing straight at `~/.aenv/envs/blank/CLAUDE.md`), and when you like what you have, capture it as a real named namespace:

```bash
cd ~/code/some-project
aenv use blank
aenv activate
$EDITOR CLAUDE.md           # edits ~/.aenv/envs/blank/CLAUDE.md via the symlink
aenv fork my-style          # clones the customized blank into a new namespace
```

`aenv fork <name>` (when `<name>` doesn't refer to a project file) clones the project's active namespace into a new one under `~/.aenv/envs/`, leaving the original untouched. After this, `blank` is back to "blank" the next time you reach for it.

### 2. Extend an existing namespace

Composition is first-class — your namespace inherits everything from its parent and overrides section-by-section:

```bash
aenv create my-style --extends karpathy
$EDITOR ~/.aenv/envs/my-style/CLAUDE.md      # adds to / overrides karpathy's content
```

Use this when an existing namespace is *most* of what you want and you just need to add or tweak a few rules.

### 3. Start from scratch

```bash
aenv create my-style --adapter claude-code
echo "## My rules" > ~/.aenv/envs/my-style/CLAUDE.md

# Then declare the file in the manifest:
#   [adapters.claude-code]
#   files = ["CLAUDE.md"]
```

`--adapter` seeds an empty adapter block in the manifest but doesn't declare any files for you — that's your call. Add `files = ["CLAUDE.md", ".claude/**/*"]` if you want a `CLAUDE.md` plus a whole `.claude/` tree.

### 4. Capture an existing project

If you've already shaped a project's `.claude/` tree and `CLAUDE.md` by hand and want to reuse that setup elsewhere, snapshot it:

```bash
cd ~/code/the-shaped-project
aenv snapshot my-existing-style
```

This walks the project, copies every adapter-managed file into a new namespace at `~/.aenv/envs/my-existing-style/`, and registers it. The project's `.aenv` pin is *not* updated — `snapshot` is a one-way capture, not an activation step.

### Iterating

Whatever path you used, the edit-test loop is the same:

```bash
cd ~/code/some-project
aenv use my-style && aenv activate
# work...
$EDITOR ~/.aenv/envs/my-style/CLAUDE.md   # symlink means edits are live; no re-activate needed
aenv status                                # confirm provenance
```

To share a namespace across machines, `git init ~/.aenv/envs/my-style && git push` — namespace directories are just files. Phase 6 adds first-class `aenv install` / `aenv sync` over git remotes.

## Reading order

- **[`pm_docs/aenv-prd.md`](./pm_docs/aenv-prd.md)** — Product requirements in EARS format. The public contract (87 requirements, R-1 through R-87).
- **[`pm_docs/aenv-functional-spec.md`](./pm_docs/aenv-functional-spec.md)** — How users interact with `aenv`. Three example harnesses, twelve user journeys, `doctor` / `diff` / scriptability examples.
- **[`pm_docs/aenv-engineering.md`](./pm_docs/aenv-engineering.md)** — Internal implementation decisions: Rust, crate selection, error / exit-code strategy, `Filesystem` trait, namespace identity model, hash specification.
- **[`tasks/todo.md`](./tasks/todo.md)** — Phase-by-phase implementation roadmap mapped back to PRD requirements.
- **[`tasks/2026-05-22-phase-3-parameters-policies.md`](./tasks/2026-05-22-phase-3-parameters-policies.md)** — Most recent implementation plan (20 tasks, bite-sized, with code and tests inline). Earlier phase plans live alongside it.
- **[`RELEASING.md`](./RELEASING.md)** — Maintainer guide for cutting a release: tag-triggered GH Actions matrix, version bump, dry-run, rollback.
- **[`INSTALL_FROM_BINARY.md`](./INSTALL_FROM_BINARY.md)** — End-user guide for installing pre-built Linux / macOS binaries from a GitHub Release (alternative to the build-from-source path in the [Installation section](#installation)).

## Building & testing

```bash
cargo build --workspace
cargo test --workspace                            # ~500 tests
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

MIT. See [`LICENSE`](./LICENSE).
