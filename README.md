# aenv — Virtual environments for AI coding harness configs

[![CI](https://github.com/Blevene/aenv/actions/workflows/ci.yml/badge.svg)](https://github.com/Blevene/aenv/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Blevene/aenv?sort=semver)](https://github.com/Blevene/aenv/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rustc-1.85+-blue.svg)](https://www.rust-lang.org)

`aenv` is a Rust CLI for managing named, composable, version-controlled bundles of AI-coding-agent configuration (`CLAUDE.md`, `.cursorrules`, `.mcp.json`, skills, agents, slash commands, MCP entries). Think Python's `venv`, but for the rules and configurations that shape how AI coding agents behave.

> **Status:** Active development. Latest release is [`v0.1.0`](https://github.com/Blevene/aenv/releases/tag/v0.1.0) — Issue #4 global namespaces (swap `~/.claude/` and other user-level harness configs). A subsequent UX-simplification pass (one-command `aenv global use`, `aenv global new`, auto-baseline) is on `main` and slated for the next release. See [§What works today](#what-works-today) for the full feature surface and [§Roadmap](#roadmap--whats-still-in-flight) for what's pending.

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

1. The existing file is **moved into `.aenv-state/backup/<timestamp>/<original-relative-path>`** (`<timestamp>` is a per-activation nanoseconds-since-epoch directory; each activation writes its own backup set). The move is recorded in `.aenv-state/state.json`.
2. The namespace's version takes its place — symlinked by default, copied or merged where the strategy demands.
3. On `aenv deactivate` (and on `aenv unpin`, which auto-deactivates), the backup is `mv`-ed back into place — byte-for-byte identical to what was there before. Files aenv *created* (paths that didn't exist pre-activation) are simply removed; no backup to restore.

Two practical consequences:

- **Editing through the symlink edits the namespace, not your original.** While a namespace is active, opening `CLAUDE.md` in your editor follows the symlink into `~/.aenv/envs/<ns>/CLAUDE.md`. The original sits in `.aenv-state/backup/<timestamp>/` until deactivate. If you want to keep edits that diverged from the namespace, capture them with `aenv fork <new-name>` before deactivating.
- **`.aenv-state/` is the safety net.** Don't delete it by hand while a namespace is active — `aenv` can't restore what it can't see. The directory disappears on its own after a clean deactivate.

### Escape hatch: `aenv restore`

If `aenv deactivate` didn't run cleanly (process killed, user deleted `state.json` by hand, force-deactivated by a script), the backup set on disk is orphaned but recoverable. `aenv restore` reads the *most recent* backup directory under `.aenv-state/backup/` and copies every file back to its original project path — overwriting anything currently there.

```bash
aenv restore        # restores .aenv-state/backup/<latest-timestamp>/* → project root
```

Restore uses *copy* semantics (vs deactivate's *move*), so the backup set is left intact — re-runnable if something goes sideways. It also works after `aenv deactivate` only if you have a stale older backup set still on disk; a clean deactivate consumes the corresponding backup, so the typical post-deactivate state is "nothing to restore."

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

## Updating a profile

Day-to-day changes on an existing namespace — adding a skill, bumping a pinned ref, editing instructions, removing things — follow a few patterns. Full step-by-step in [`docs/walkthroughs/updating-a-profile.md`](./docs/walkthroughs/updating-a-profile.md). The cheat sheet:

```bash
# Add a skill (authored or imported). For a GLOBAL profile, pass --scope user
# so the skill materializes into ~/.claude/skills/ on `aenv global use`.
aenv skill new <name> --ns <profile> [--scope user]
aenv skill import git+<url> --ns <profile> --pin <ref> [--path <subdir>] [--scope user]

# Edit existing content (CLAUDE.md, an existing SKILL.md, etc.)
$EDITOR ~/.aenv/envs/<profile>/CLAUDE.md           # live via symlink; no re-activate

# Bump a pinned skill's ref
$EDITOR ~/.aenv/envs/<profile>/aenv.toml           # change ref = "<new>"

# Remove a skill
aenv skill remove <name> --ns <profile>            # manifest + on-disk dir for authored

# Remove a file (no CLI command yet — manual two-step)
$EDITOR ~/.aenv/envs/<profile>/aenv.toml           # delete the files[] entry
rm ~/.aenv/envs/<profile>/<path>

# Reclaim cache space (orphaned imported-skill clones)
aenv cache prune
```

**The gotcha**: any change that adds or removes a managed path (new skill, new file, removed skill, bumped pin) requires `aenv deactivate && aenv activate` in every project where the namespace is currently active. Edits to existing files are live via the symlink and need no re-activate.

### Agent-side guidance: the `aenv` skill

If you'd rather not memorize the CLI surface, this repo ships its own Claude Code skill that gives an agent the user-request-to-command map plus the gotchas. Import it into any namespace you're using in a project:

```bash
aenv skill import git+https://github.com/Blevene/aenv \
    --ns <your-namespace> \
    --path skills/aenv \
    --pin v0.0.3
```

On the next `aenv activate`, the skill materializes at `.claude/skills/aenv/SKILL.md` in your project, and Claude Code's agent will load it when you ask aenv-shaped questions ("switch profile", "install a skill", "auto-activate on cd," etc.).

## Global namespaces

### What it does

`aenv global` moves user-scope harness files (`~/.claude/CLAUDE.md`, `~/.claude/agents/`, `~/.codex/AGENTS.md`, …) in and out of `$HOME` under a single named activation. Files declared in a namespace's `user_files` are materialized from `~/.aenv/envs/<ns>/user/`; collisions with pre-existing files are stashed to `~/.aenv/global-stash/<ts>/` and restored byte-for-byte on deactivate. If the namespace ships an `on_activate` script, aenv runs it after materialization (with first-run approval pinned to the script's sha256). Everything else under `$HOME` — files no namespace declares — is yours: aenv neither touches it nor knows about it.

### Onboard a profile in one command

```bash
# Onboard claude-ctrl from upstream: import its config AND activate it in one
# shot. On your very first global activation, aenv also captures your current
# ~/ surface as a 'baseline' namespace so you always have a return point.
# (A namespace can declare a lifecycle hook in aenv-namespace.toml to run a
# runtime installer on activation; the first run prompts for approval. The
# heuristic does not auto-run a repo's install.sh — see Lifecycle hooks below.)
aenv global use https://github.com/juanandresgs/claude-ctrl

# Swap back to your captured baseline (or any namespace by name).
aenv global use baseline

# Toggle back to the profile you were just on.
aenv global use -
```

`aenv global use <target>` is the front door: `<target>` is a git URL or local path (imported on the spot if not already present, then activated), an existing namespace name (switch), or `-` (toggle to the previous profile). Switching deactivates whatever is currently active and activates the target in one transaction; if it fails, the prior one is restored as best-effort rollback. One global activation lives per user, full stop.

To author your own profile from scratch instead of importing one:

```bash
aenv global new mine        # scaffold an editable ~/.aenv/envs/mine/user/.claude/CLAUDE.md
$EDITOR ~/.aenv/envs/mine/user/.claude/CLAUDE.md
aenv global use mine
```

### Verbs at a glance

| Command | Purpose |
|---|---|
| `aenv global use <target> [--as <name>] [--pin <ref>] [--yes] [--no-baseline]` | **The front door.** `<target>` = git URL / local path (import + activate), an existing namespace name (switch), or `-` (previous profile). |
| `aenv global new <name> [--adapter <a>]` | Scaffold a new, editable user-scope namespace from scratch (seeds the adapter's instructions file + a pre-wired manifest). |
| `aenv global snapshot <name> [--include <path>...]` | Capture the current `$HOME` user-scope surface into a new namespace. |
| `aenv global import <source> [<name>] [--pin <ref>]` | Lower-level import (no activation): turn a local path or git URL into a namespace. Imports config files (heuristic, or per the source's `aenv-namespace.toml`). Lifecycle hooks are opt-in via `aenv-namespace.toml` only — a bare `install.sh` is not auto-wired. |
| `aenv global activate <ns> [--yes] [--no-baseline]` | **Deprecated** — use `aenv global use <ns>`. Still works (prints a notice); equivalent to `use` on an existing namespace, minus source-import. |
| `aenv global deactivate [--force]` | Restore the pre-activation `$HOME` surface. `--force` skips a broken `on_deactivate`; file restoration runs either way. |
| `aenv global status [--json]` | Show the active namespace + every managed `~/<path>`. |
| `aenv global which <path> [--json]` | "Which namespace manages `~/.claude/foo`?" — JSON includes the file's `content_hash`. |
| `aenv global list [--json]` | List every namespace whose manifest declares `user_files`. |
| `aenv global doctor [<ns>] [--json] [--fix]` | Run policies (`instructions_max_chars`, `hook_paths_resolvable`, `copy_mode_local_edits`) against user-scope candidates; flag orphan stashes (exit 19). `--fix` clears the orphan stashes it finds. |
| `aenv global diff [<a> <b>] [--json]` | Byte-level drift detection (no args) or structural diff between two namespaces' user-scope subsets. |
| `aenv use <ns> --global [--yes]` | Sugar: pin the project, activate it, and activate `<ns>` globally — all in one command. |

### Lifecycle hooks: when to use them

If your namespace needs to install a runtime (e.g. claude-ctrl's Python policy engine), declare `[lifecycle] on_activate = "install.sh"` in its `aenv.toml` — or, for a repo intended to be imported, in its `aenv-namespace.toml`. Lifecycle hooks are **opt-in and explicit**: aenv never infers them from a bare `install.sh` during a heuristic import, because a repo's installer typically wants to own `~/.claude` itself and would fight aenv's materialization. A declared `on_activate` runs after materialization with `cwd = $HOME` and env vars `AENV_NAMESPACE` / `AENV_NAMESPACE_DIR` / `AENV_TARGET_ROOT` set, and should do runtime-only setup (aenv already placed the files). First activation prompts before running; approvals are pinned to the script's sha256 at `~/.aenv/envs/<ns>/.approved`, so future edits re-prompt. Full contract in [`pm_docs/lifecycle-hooks.md`](./pm_docs/lifecycle-hooks.md).

### Extending the adapter surface

A namespace's `user_files` is not capped by what its adapter declares. claude-ctrl, for example, declares `.claude/runtime/` in its own manifest even though the builtin claude-code adapter doesn't — aenv materializes any user-scoped path the namespace asks for, as long as it's relative and doesn't escape with `..`.

### Editing a live activation: where your edits go

Two materialization modes, picked per adapter or per namespace via `materialize = "symlink"` (default) or `materialize = "copy"`:

- **Symlink mode (default).** `~/.claude/CLAUDE.md` is a symlink into `~/.aenv/envs/<ns>/user/.claude/CLAUDE.md`. Opening it in your editor edits the namespace source directly — and re-activating the same namespace shows the same edits because they were never separate.
- **Copy mode.** `~/.claude/CLAUDE.md` is a regular file copied from the namespace source at activation. Edits stick to the working copy; the namespace source is untouched. The next `aenv global use <same-ns>` overwrites your edits without warning.

If you want to keep edits made under copy mode, run `aenv global snapshot <new-name>` first to capture the current `$HOME` state into a fresh namespace. `aenv global doctor` warns when a copy-mode target has drifted from its source.

### Recovery: when things go wrong

Two escape hatches:

- **`aenv global deactivate --force`** — skips the namespace's `on_deactivate` lifecycle hook. Use when that hook is itself broken. File restoration runs either way.
- **`aenv-rescue`** — a standalone binary (no `aenv` dependency) that reads `~/.aenv/global-state.json` directly, undoes the activation via fs ops, and never invokes lifecycle scripts. Use when the main aenv binary or its lifecycle scripts have locked you out of a Claude Code session via a broken PreToolUse hook.

Full recovery flow in [`pm_docs/walkthrough-global-namespaces.md`](./pm_docs/walkthrough-global-namespaces.md).

### What aenv does NOT do

- **Generic package installation.** `on_activate` can run `pip install` or `brew install`, but aenv doesn't know pip from npm from apt — it just runs your script.
- **Live-reload of running harness processes.** Claude Code, Codex, Cursor read their config at startup. Restart the harness to pick up a new activation.
- **Process-tree isolation.** One global activation lives per user. Two terminals can't each see a different active namespace.
- **Live-edit conflict resolution.** Files no namespace declares are yours; aenv won't touch them, won't back them up, won't merge them. If you want a file managed, declare it in `user_files`.

## Shell integration

After authoring some namespaces, you'll get tired of typing `aenv use && aenv activate` every time you `cd` into a project. The shell hook automates that — `cd` between pinned projects auto-activates the right namespace; `cd` to anywhere unpinned auto-deactivates. Full walkthrough in [`docs/walkthroughs/shell-integration.md`](./docs/walkthroughs/shell-integration.md).

```bash
# zsh — add to ~/.zshrc
eval "$(aenv init-shell zsh)"

# bash — add to ~/.bashrc
eval "$(aenv init-shell bash)"

# fish — add to ~/.config/fish/config.fish
aenv init-shell fish | source
```

On every `chpwd`, the hook calls `aenv activate-if-needed "$_AENV_ACTIVE"` — which walks the cwd's ancestors for a `.aenv` pin, compares to the previous active project, and transitions only when needed. The no-change path is just an ancestor walk + string compare (sub-millisecond), so the hook is safe to run on every prompt.

Track which project the hook thinks is active with the `_AENV_ACTIVE` env var.

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

## What works today

`aenv` can:

- **Create and compose namespaces.** A namespace bundles `CLAUDE.md`, `.cursorrules`, skills, agents, settings — anything an AI coding harness reads — and can `extends` another namespace. Composition produces section-merged Markdown, deep-merged JSON / YAML / TOML, and qualified-name provenance for every artifact. Cycles are caught (exit 15).
- **Pin and activate projects.** `aenv use <name>` writes a `.aenv` pin file; `aenv activate` materializes the resolved namespace as symlinks (or merged files where strategy demands) and records every move in `.aenv-state/state.json`. `aenv deactivate` puts the project back exactly as it was, restoring any files it displaced.
- **Inspect provenance.** `aenv status` shows the resolution chain, every managed file with its qualified source, the shadow chain, effective parameters, and active policies. `aenv which <path>` answers "where did this file come from?".
- **Declare typed parameters and policies.** Manifests carry `[parameters]` (string / int / bool / list-of-string) that inherit last-wins across the extends chain, and `[policies]` (advisory by default, or `enforce = true`) that inherit with R-75 enforce-protection — a child can tighten but not weaken a parent's enforced policy.
- **Run a doctor check.** `aenv doctor [<ns>]` evaluates four built-in policy evaluators (`instructions_max_chars`, `skill_requires_description`, `mcp_requires_command_or_url`, `forbid_paths`) against the resolved namespace and prints per-policy outcomes. Enforced violations also block `aenv activate` with exit 17 — *before* any file is touched.
- **Read and write parameters from the CLI.** `aenv get <ns>.<param>` or `aenv get .<param>` (active project) shows the effective value with provenance; `aenv set <ns>.<param> <value>` rewrites the named namespace's manifest, inferring the value type.
- **Fork to a private copy.** `aenv fork` detaches a whole project from its namespace (replacing symlinks with copies); `aenv fork <file>` detaches just one file; `aenv fork <name>` creates a new namespace populated from the current project state.
- **Manage skills.** `aenv skill new <name> --ns <ns>` scaffolds an authored skill whose files live in the namespace tree; `aenv skill import <source> --ns <ns>` pulls one in from a local path or git URL, with `--pin <ref>` for reproducibility and `--path <subdir>` for monorepo skill collections (k-dense-ai's `scientific-agent-skills`, etc.). `aenv skill remove <name> --ns <ns>` undoes either flavor; `aenv cache prune` reclaims `~/.aenv/cache/skills/` dirs nothing references.
- **Snapshot an existing project.** `aenv snapshot <name>` walks the project against every installed adapter's `files = [...]` and copies the matches into a new namespace at `~/.aenv/envs/<name>/` — useful for capturing a hand-shaped `.claude/` tree as something portable.
- **Diff resolved namespaces.** `aenv diff` compares a project's materialized files against the namespace declaration (drift detection); `aenv diff <ns_a> <ns_b>` shows the structural delta between two namespaces. Both ship `--json` for downstream tooling.
- **Scriptability.** Every read-oriented command — `list`, `status`, `which`, `get`, `doctor`, `skill list`, `adapter list`, `diff` — accepts `--json` and emits a stable schema. Each namespace also carries a resolved-namespace hash you can read off `aenv status` / `aenv list --json` to compare configurations across machines.

Ships with built-in adapters for **Claude Code, Cursor, Aider, Cline, Continue, Windsurf, Codex, and a generic MCP adapter** — all embedded in the binary, written to `~/.aenv/adapters/` on first run, and overridable by user edit. `aenv adapter list` shows what's installed; `aenv adapter add <path>` registers a new one. Also ships with two starter namespaces (`karpathy`, `cherny`) written to `~/.aenv/envs/` on first run so you have something to switch between out of the box.

## Roadmap — what's still in flight

The full plan lives in [`tasks/todo.md`](./tasks/todo.md). Two phases remain:

- **Phase 6** — Partial. `cd`-based auto-activation ships now via `aenv init-shell` (see [§Shell integration](#shell-integration)); git remotes / `aenv install` / `aenv sync` / `aenv promote` still pending.
- **Phase 7** — Windows symlink fallback, cross-platform CI, v0.1.0 release.

## Reading order

- **[`pm_docs/aenv-prd.md`](./pm_docs/aenv-prd.md)** — Product requirements in EARS format. The public contract (87 requirements, R-1 through R-87).
- **[`pm_docs/aenv-functional-spec.md`](./pm_docs/aenv-functional-spec.md)** — How users interact with `aenv`. Three example harnesses, twelve user journeys, `doctor` / `diff` / scriptability examples.
- **[`pm_docs/aenv-engineering.md`](./pm_docs/aenv-engineering.md)** — Internal implementation decisions: Rust, crate selection, error / exit-code strategy, `Filesystem` trait, namespace identity model, hash specification.
- **[`tasks/todo.md`](./tasks/todo.md)** — Phase-by-phase implementation roadmap mapped back to PRD requirements.
- **[`tasks/2026-05-22-phase-3-parameters-policies.md`](./tasks/2026-05-22-phase-3-parameters-policies.md)** — Most recent implementation plan (20 tasks, bite-sized, with code and tests inline). Earlier phase plans live alongside it.
- **[`RELEASING.md`](./RELEASING.md)** — Maintainer guide for cutting a release: tag-triggered GH Actions matrix, version bump, dry-run, rollback.
- **[`INSTALL_FROM_BINARY.md`](./INSTALL_FROM_BINARY.md)** — End-user guide for installing pre-built Linux / macOS binaries from a GitHub Release (alternative to the build-from-source path in the [Installation section](#installation)).
- **[`docs/walkthroughs/`](./docs/walkthroughs/)** — Step-by-step recipes for common journeys: [setup + first swap](./docs/walkthroughs/setup-and-first-swap.md), [build your own namespace](./docs/walkthroughs/build-your-own.md), [install a skill from GitHub](./docs/walkthroughs/install-a-skill-from-github.md), [snapshot an existing project](./docs/walkthroughs/snapshot-an-existing-project.md), [update an existing profile](./docs/walkthroughs/updating-a-profile.md), [shell integration (cd-based auto-activation)](./docs/walkthroughs/shell-integration.md).

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
