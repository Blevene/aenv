# Walkthrough: import a global profile from GitHub (ECC)

Goal: turn a GitHub repo of agent-harness config into a **global profile** — one
named bundle that materializes into your home directory (`~/.claude/`,
`~/.codex/`, …) and can be swapped or rolled back as a unit. The worked example
is [`affaan-m/ECC`](https://github.com/affaan-m/ECC), "the agent harness
performance optimization system" — a repo that ships `CLAUDE.md`, `AGENTS.md`,
`.claude/`, and `.codex/` exactly the way aenv's user scope expects.

> **New to aenv?** A **namespace** is a named bundle of harness config. *Global*
> (user) scope materializes it into `~/.claude/`, `~/.codex/`, … for every
> project, as opposed to *project* scope which writes into one repo's `.claude/`.
> `aenv global use` *materializes* a profile (symlinks its files into `$HOME`).
> Full [glossary](./README.md#glossary).

> ⚠️ **This touches your real home directory.** Unlike the project-scope
> walkthroughs, global activation writes into `~/.claude/` and `~/.codex/`. aenv
> backs up (stashes) any file it would displace and restores it on deactivate —
> but if you just want to try it without touching your setup, run everything
> below with a scratch home: `export HOME=$(mktemp -d)` in a throwaway shell.

## Prerequisites

- `aenv` installed (see the [README](../../README.md#installation)) and `git` on `PATH`.
- Network access (the repo is cloned on import).

## Step 1: Import and activate in one command

`aenv global use <git-url>` is the front door: it imports the source, captures a
first-run baseline, and activates — in one step.

```bash
aenv global use https://github.com/affaan-m/ECC --as ECC --yes
```

```
Imported 'ECC' from https://github.com/affaan-m/ECC (commit 64cd1ba248e77e377e76f70fc4e6434bfdddd511, 2 files, 5 directories captured; via heuristic layout).
  + .claude/CLAUDE.md
  + .claude/agents/
  + .claude/commands/
  + .claude/hooks/
  + .claude/skills/
  + .codex/
  + .codex/AGENTS.md
Activated 'ECC' globally in /home/you
  + .claude/CLAUDE.md (Symlink)
  + .claude/agents (Symlink)
  + .claude/commands (Symlink)
  + .claude/hooks (Symlink)
  + .claude/skills (Symlink)
  + .codex (Symlink)
  + .codex/AGENTS.md (Identical)
Note: running harness sessions retain their previous config until restart.
```

What happened:

- **`--as ECC`** names the imported namespace `ECC` (otherwise it derives the
  name from the repo).
- **`--yes`** is non-interactive: it approves any lifecycle prompts and proceeds
  past pre-flight findings without asking. Drop it to review each step.
- The repo had no `aenv-namespace.toml`, so aenv used its **heuristic layout** —
  it probes well-known config paths and imports *config only* (it never wires a
  repo's `install.sh` as a lifecycle hook).
- The first global activation captures a **baseline** of your current `$HOME`
  config so you can roll back (opt out with `--no-baseline`). On a machine with
  no existing `~/.claude`, there's nothing to capture, so no baseline namespace
  appears.

> **URL form.** `global use` / `global import` accept a bare `https://` URL (or
> `http://`, `git://`, `git@`, `file://`, or any URL ending in `.git`). As of
> **v0.3.3** the `git+https://…` prefix that `aenv skill import` uses is also
> accepted (it's stripped before cloning), so either form works. Use
> `--pin <tag|branch|sha>` to lock a specific commit.

## Step 2: Confirm what's active

```bash
aenv global status
```

```
Active global namespace: ECC
Target root: /home/you
Managed files: 7
  ~/.claude/CLAUDE.md
  ~/.claude/agents
  ~/.claude/commands
  ~/.claude/hooks
  ~/.claude/skills
  ~/.codex
  ~/.codex/AGENTS.md
Note: running harness sessions retain their previous config until restart.
```

## Step 3: See what was captured (and what wasn't)

The import wrote a manifest at `~/.aenv/envs/ECC/aenv.toml`:

```bash
cat ~/.aenv/envs/ECC/aenv.toml
```

```toml
name = "ECC"
extends = []

[adapters.claude-code]
files = []
user_files = [
    ".claude/CLAUDE.md",
    ".claude/agents/",
    ".claude/commands/",
    ".claude/hooks/",
    ".claude/skills/",
]

[adapters.codex]
files = []
user_files = [
    ".codex/",
    ".codex/AGENTS.md",
]
```

The heuristic captured only what maps to a built-in adapter's **user-scope**
patterns: Claude Code (`~/.claude/CLAUDE.md` + the `agents/`, `commands/`,
`hooks/`, `skills/` subdirs) and Codex (`~/.codex/` + `AGENTS.md`). ECC's other
directories — `.cursor/`, `.gemini/`, `.qwen/`, `.zed/`, `.mcp.json`, … — were
**not** imported, because no built-in adapter declares a user-scope path for
them (`.mcp.json`, for instance, is a *project*-scope file only). aenv manages
exactly what it understands and leaves the rest alone.

Confirm ownership of any path with `which`:

```bash
aenv global which '~/.claude/CLAUDE.md'
aenv global which '~/.codex/AGENTS.md'
```

```
~/.claude/CLAUDE.md -> ECC::.claude/CLAUDE.md
~/.codex/AGENTS.md -> ECC::.codex/AGENTS.md
```

(Quote the `~/` path, or pass the absolute `$HOME`-rooted path — both resolve.)

## Step 4: Health-check the profile

```bash
aenv global doctor ECC
```

```
[PASS] instructions_max_chars ECC::~/.claude/CLAUDE.md
[WARN] instructions_max_chars ECC::~/.codex/AGENTS.md — .codex/AGENTS.md has 5507 chars (budget 5000). Refactor procedural content into skills, dispositional content into subagents, or use @-imports.
```

`doctor` evaluates each adapter's policies against the profile's content. Here
ECC's `~/.claude/CLAUDE.md` is within the instructions budget, while its
`~/.codex/AGENTS.md` is slightly over the 5000-char soft limit — a non-blocking
`WARN` with a concrete remediation hint. (These numbers reflect ECC's content at
the imported commit and will shift as the repo changes.)

## Step 5: See how it's wired on disk

Every managed path is a symlink back into the namespace's `user/` subtree:

```bash
ls -l ~/.claude
```

```
CLAUDE.md -> ~/.aenv/envs/ECC/user/.claude/CLAUDE.md
agents    -> ~/.aenv/envs/ECC/user/.claude/agents
commands  -> ~/.aenv/envs/ECC/user/.claude/commands
hooks     -> ~/.aenv/envs/ECC/user/.claude/hooks
skills    -> ~/.aenv/envs/ECC/user/.claude/skills
```

Because they're symlinks, editing a file under `~/.aenv/envs/ECC/user/` (or
through the symlink) takes effect immediately — no re-activate needed.

## Turning it off / swapping

```bash
aenv global deactivate
```

```
Deactivated namespace 'ECC' globally in /home/you
```

This removes the symlinks, restores anything that was stashed during activation,
and clears the active global state. The `ECC` namespace stays in your registry:

```bash
aenv global list
```

```
ECC
```

To switch to a different global profile, just `aenv global use <other>`; aenv
deactivates ECC and activates the other in a single transaction. `aenv global
use -` toggles back to the previous profile.

## If something goes wrong

- **`namespace not found: <url>`** — the target wasn't recognized as a git URL.
  Check the scheme: use `https://…`, `git@…`, `git+https://…` (v0.3.3+), or a
  URL ending in `.git`.
- **Undo everything:** `aenv global deactivate` restores your prior `$HOME`
  config (from the stash) and removes ECC's symlinks. The import itself only
  added a namespace under `~/.aenv/envs/ECC/` — delete it with
  `aenv delete ECC` (after deactivating) to discard it entirely.
- **A path you expected wasn't captured:** it has no user-scope adapter mapping
  (see Step 3). You can add it by editing `~/.aenv/envs/ECC/aenv.toml` and
  re-activating.

## What to read next

- [updating-a-profile](./updating-a-profile.md) — edit ECC after import: add
  skills, bump pins, change instructions.
- [README §Global namespaces](../../README.md#global-namespaces) — the full
  model: baselines, stashing, lifecycle hooks, and recovery with `aenv-rescue`.
