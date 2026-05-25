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
- **Manage skills.** `aenv skill new <name> --ns <ns>` scaffolds an authored skill whose files live in the namespace tree; `aenv skill import <source> --ns <ns>` pulls one in from a local path or git URL, with `--pin <ref>` for reproducibility and `--path <subdir>` for monorepo skill collections (k-dense-ai's `scientific-agent-skills`, etc.).

Ships with built-in adapters for **Claude Code, Cursor, Aider, Cline, Continue, Windsurf, Codex, and a generic MCP adapter** — all embedded in the binary, written to `~/.aenv/adapters/` on first run, and overridable by user edit. Also ships with two starter namespaces (`karpathy`, `cherny`) written to `~/.aenv/envs/` on first run so you have something to switch between out of the box.

## What's still in flight

The roadmap (see [`tasks/todo.md`](./tasks/todo.md)) has three phases left:

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

For a step-by-step setup-to-first-swap walkthrough, see [`docs/walkthroughs/setup-and-first-swap.md`](./docs/walkthroughs/setup-and-first-swap.md).

## Try the built-in namespaces

`aenv` ships with two starter namespaces — `karpathy` (surgical, "minimum code to solve the problem") and `cherny` (plan-first, subagent-heavy) — both wired up against the `claude-code` adapter and materialized to `~/.aenv/envs/` automatically on first run. Use them to see the activate / switch / restore loop without authoring anything yourself.

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

## What happens to your existing files

`aenv` only touches files when you run `aenv activate`. Until then — and after `aenv deactivate` — your project is exactly what you left it. A hand-authored `CLAUDE.md` (or `.cursorrules`, or anything else a namespace might manage) sits untouched if you've never activated.

When activation finds a file it's about to manage:

1. The existing file is **renamed to `.aenv-state/backups/<file>.<timestamp>`** and the rename is recorded in `.aenv-state/state.json`.
2. The namespace's version takes its place — symlinked by default, copied or merged where the strategy demands.
3. On `aenv deactivate` (and on `aenv unpin`, which auto-deactivates), the backup is `mv`-ed back into place — byte-for-byte identical to what was there before. Files aenv *created* (paths that didn't exist pre-activation) are simply removed; no backup to restore.

Two practical consequences:

- **Editing through the symlink edits the namespace, not your original.** While a namespace is active, opening `CLAUDE.md` in your editor follows the symlink into `~/.aenv/envs/<ns>/CLAUDE.md`. The original sits in `.aenv-state/backups/` until deactivate. If you want to keep edits that diverged from the namespace, capture them with `aenv fork <new-name>` before deactivating.
- **`.aenv-state/` is the safety net.** Don't delete it by hand while a namespace is active — `aenv` can't restore what it can't see. The directory disappears on its own after a clean deactivate.

## Creating your own namespace

Three common starting points, in roughly increasing order of "I have something in mind."

### 1. Start from scratch

Full step-by-step in [`docs/walkthroughs/build-your-own.md`](./docs/walkthroughs/build-your-own.md). The short version:

```bash
aenv create my-style --adapter claude-code
$EDITOR ~/.aenv/envs/my-style/CLAUDE.md   # blank file, ready to write
```

`--adapter <name>` scaffolds a usable namespace: it declares `[adapters.<name>]` in the manifest *and* creates empty versions of every concrete file the adapter manages (e.g. `CLAUDE.md` for `claude-code`, `.cursorrules` for `cursor`, `.mcp.json` for `mcp`). The manifest's `files = [...]` is populated to match, so `aenv activate` immediately materializes those files into your project without any further edits to `aenv.toml`.

To add a skill, see [§ Skills](#skills) — `aenv skill new <skill> --ns my-style` scaffolds an authored skill under `.claude/skills/<skill>/`.

### 2. Extend an existing namespace

Composition is first-class — your namespace inherits everything from its parent and overrides section-by-section:

```bash
aenv create my-style --extends karpathy
$EDITOR ~/.aenv/envs/my-style/CLAUDE.md      # adds to / overrides karpathy's content
```

Use this when an existing namespace is *most* of what you want and you just need to add or tweak a few rules. Combine with `--adapter` if you also want fresh adapter scaffolding on top of the inheritance chain.

### 3. Capture an existing project

Full step-by-step in [`docs/walkthroughs/snapshot-an-existing-project.md`](./docs/walkthroughs/snapshot-an-existing-project.md). The short version:

```bash
cd ~/code/the-shaped-project
aenv snapshot my-existing-style
```

aenv walks the project against every installed adapter's declared `files = [...]` patterns — globs, trailing-slash directory markers, and literal paths all expand to concrete files — then copies each match into `~/.aenv/envs/my-existing-style/` and writes a manifest that declares every captured path explicitly.

**Before — a project shaped by hand:**

```
~/code/the-shaped-project/
├── CLAUDE.md                                 # your working agreements
├── .claude/
│   ├── skills/linter-discipline/SKILL.md
│   └── commands/review.md
└── .mcp.json                                 # MCP server config
```

**After — the captured namespace:**

```
~/.aenv/envs/my-existing-style/
├── aenv.toml                                 # [adapters.claude-code].files = [..every path expanded..]
├── CLAUDE.md
├── .claude/skills/linter-discipline/SKILL.md
├── .claude/commands/review.md
└── .mcp.json
```

The project's `.aenv` pin is *not* updated — `snapshot` is a one-way capture, not an activation step. To reuse the captured namespace elsewhere:

```bash
cd ~/other-project
aenv use my-existing-style && aenv activate
```

Skills captured this way are recorded as `mode = "authored"` (files live in the namespace tree itself, so the snapshot is self-contained). If you'd rather track a skill against an upstream git repo, edit the manifest to flip its `[[skills]]` entry to `mode = "imported"` and add `source = "git+https://..."` — `aenv` will then fetch on the next activation instead of using the captured copy.

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

## Skills

Skills are reusable instruction bundles — typically a `SKILL.md` with YAML frontmatter, plus any supporting `references/`, `scripts/`, `assets/` — that the agent should invoke for specific tasks. aenv manages them as part of a namespace's content: when you activate the namespace, every declared skill materializes under the adapter's `skills_dir` in your project (`.claude/skills/<name>/` for the claude-code adapter).

Two flavors, distinguished by where the files live.

### Authored skills — `aenv skill new`

Files live under the namespace's own directory. Use this when you're writing your own skills from scratch.

```bash
aenv skill new my-checker --ns my-style
# Creates ~/.aenv/envs/my-style/.claude/skills/my-checker/SKILL.md
# and adds the [[skills]] entry to my-style's aenv.toml.
$EDITOR ~/.aenv/envs/my-style/.claude/skills/my-checker/SKILL.md
```

When `my-style` is activated in a project, `my-checker/SKILL.md` materializes at `.claude/skills/my-checker/SKILL.md` (symlinked to the namespace dir, so edits are live).

### Imported skills — `aenv skill import`

Full step-by-step in [`docs/walkthroughs/install-a-skill-from-github.md`](./docs/walkthroughs/install-a-skill-from-github.md). The reference:

Files live somewhere external — a local path or a git URL — and are fetched at activation time, cached under `~/.aenv/cache/skills/<source-hash>/<ref>/`. Use this when you're pulling in someone else's skill.

```bash
# From a git repo whose SKILL.md sits at the root
aenv skill import git+https://github.com/example/some-skill --ns my-style --pin v1.0

# From a local path
aenv skill import ~/code/skills/notes --ns my-style

# From a monorepo: pick one skill by its sub-path
aenv skill import git+https://github.com/k-dense-ai/scientific-agent-skills \
    --ns my-style \
    --path scientific-skills/scanpy \
    --pin v2.39.0
```

The `--path` flag is the one to reach for when the source repo bundles many skills under a directory (k-dense-ai's layout is `scientific-skills/<name>/SKILL.md`; other community skill collections use similar conventions). Without `--path`, aenv looks for `SKILL.md` at the cache root or under `<skill_name>/`; with it, aenv looks under the named sub-directory and the skill name defaults to that sub-directory's basename. Path values must be relative and free of `..` segments.

`--pin <ref>` resolves once at import time and locks the recorded commit SHA in the manifest, so activations are reproducible across machines. Omit it to re-resolve to `HEAD` on each activation.

### What import + activate actually do

`aenv skill import` writes the manifest entry but does *not* fetch content yet — that happens on the next `aenv activate`. The full sequence:

1. **Import** — `aenv skill import …` validates the source format, derives the skill name (from `--path` basename, URL fragment, or repo name), and appends a `[[skills]]` entry to the namespace's `aenv.toml`. With `--pin`, it does a one-shot resolution to verify reachability and lock the commit SHA.
2. **Activate** — for each imported skill not yet cached, aenv shallow-clones the source into `~/.aenv/cache/skills/<source-hash>/<ref>/`. Repeats hit the cache; pinned refs are immutable, so they only fetch once per machine.
3. **Materialize** — the skill's entire directory tree under the source (`SKILL.md` plus any `references/`, `scripts/`, `assets/`) is symlinked into your project at `.claude/skills/<skill_name>/`. The `--path` sub-tree, if specified, scopes which files come along.
4. **Inspect** — `aenv status` reports each managed file's source URL, resolved commit SHA, and content hash, so you can verify reproducibility across machines.

### Listing what's declared

```bash
aenv skill list                # every skill across every namespace
aenv skill list --ns my-style  # just one namespace's
aenv skill list --json         # machine-readable
```

## Reading order

- **[`pm_docs/aenv-prd.md`](./pm_docs/aenv-prd.md)** — Product requirements in EARS format. The public contract (87 requirements, R-1 through R-87).
- **[`pm_docs/aenv-functional-spec.md`](./pm_docs/aenv-functional-spec.md)** — How users interact with `aenv`. Three example harnesses, twelve user journeys, `doctor` / `diff` / scriptability examples.
- **[`pm_docs/aenv-engineering.md`](./pm_docs/aenv-engineering.md)** — Internal implementation decisions: Rust, crate selection, error / exit-code strategy, `Filesystem` trait, namespace identity model, hash specification.
- **[`tasks/todo.md`](./tasks/todo.md)** — Phase-by-phase implementation roadmap mapped back to PRD requirements.
- **[`tasks/2026-05-22-phase-3-parameters-policies.md`](./tasks/2026-05-22-phase-3-parameters-policies.md)** — Most recent implementation plan (20 tasks, bite-sized, with code and tests inline). Earlier phase plans live alongside it.
- **[`RELEASING.md`](./RELEASING.md)** — Maintainer guide for cutting a release: tag-triggered GH Actions matrix, version bump, dry-run, rollback.
- **[`INSTALL_FROM_BINARY.md`](./INSTALL_FROM_BINARY.md)** — End-user guide for installing pre-built Linux / macOS binaries from a GitHub Release (alternative to the build-from-source path in the [Installation section](#installation)).
- **[`docs/walkthroughs/`](./docs/walkthroughs/)** — Step-by-step recipes for common journeys: [setup + first swap](./docs/walkthroughs/setup-and-first-swap.md), [build your own namespace](./docs/walkthroughs/build-your-own.md), [install a skill from GitHub](./docs/walkthroughs/install-a-skill-from-github.md), [snapshot an existing project](./docs/walkthroughs/snapshot-an-existing-project.md), [update an existing profile](./docs/walkthroughs/updating-a-profile.md).

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
