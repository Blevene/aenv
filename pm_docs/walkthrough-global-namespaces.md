# Walkthrough: global namespaces

**Tested against:** `main` (commit `3100d80`), `aenv 0.0.3`.
**Goal:** activate one namespace's user-scope surface under `$HOME`, swap to another in a single transaction, catch a too-large user CLAUDE.md with `aenv global doctor`, and clean up an orphan stash with `aenv global deactivate --prune`.

A project-scope namespace owns files under the project root (`./CLAUDE.md`, `./.claude/skills/…`, `./.mcp.json`, …). A **global** namespace owns files under the user's `$HOME` — `~/.claude/CLAUDE.md`, `~/.claude/agents/`, `~/.codex/AGENTS.md`, etc. The two surfaces can be expressed by the same namespace: the project files come from `~/.aenv/envs/<ns>/`, the user files from `~/.aenv/envs/<ns>/user/`. `aenv activate` swaps the first; `aenv global activate` swaps the second.

This walkthrough exercises every `aenv global` verb end-to-end against an isolated `$HOME` so nothing touches your real `~/.claude/`.

For the project-scope walkthrough see [`walkthrough-three-harnesses.md`](./walkthrough-three-harnesses.md).

---

## Prerequisites

Build the release binary:

```bash
cargo build --release
```

Pick scratch directories so the walkthrough never touches your real registry or your real `~/.claude/`:

```bash
export AENV_HOME=$(mktemp -d -t aenv-global-XXXXXX)/.aenv
export HOME=$(mktemp -d -t aenv-home-XXXXXX)
export BIN=$PWD/target/release/aenv

mkdir -p "$HOME"
$BIN --version
# → aenv 0.0.3
```

Two paths to keep in your head:

- **`$HOME`** — the surface global activations materialize into. `aenv global activate <ns>` writes (symlinks) files under here.
- **`$AENV_HOME`** — where the namespace registry lives, and also where the per-user **global activation state** is stored: `$AENV_HOME/global-state.json` (schema v5), backups under `$AENV_HOME/global-stash/<timestamp>/`, the cross-process lock at `$AENV_HOME/global.lock`.

Confirm the starting state:

```bash
$BIN global status
```

```
no global activation
```

Same in JSON:

```bash
$BIN global status --json
```

```json
{"active":false,"scope":"user"}
```

---

## Step 1 — build a `research` namespace with user-scope content

`aenv create` writes the manifest skeleton; the rest is hand-authored (the `user/` subdirectory and the `user_files = [...]` manifest entry are both manual today — there's no `aenv skill new --scope user` equivalent yet).

```bash
$BIN create research --adapter claude-code
```

```
Created namespace 'research' at /tmp/aenv-global-XXXXXX/.aenv/envs/research
```

Author three pieces of user-scope content under `~/.aenv/envs/research/user/` — the directory layout mirrors the materialization target under `$HOME`:

```bash
mkdir -p $AENV_HOME/envs/research/user/.claude/agents
cat > $AENV_HOME/envs/research/user/.claude/CLAUDE.md <<'EOF'
# Research mode
Prefer exploratory analysis; cite primary sources.
EOF
cat > $AENV_HOME/envs/research/user/.claude/agents/explorer.md <<'EOF'
---
name: explorer
description: Explores datasets and surfaces patterns.
---
EOF
echo '{"defaultModel":"claude-opus-4.7"}' \
  > $AENV_HOME/envs/research/user/.claude/settings.json
```

Now edit the manifest to declare which user-scope files this namespace owns. Paths are written **without** the `~/` prefix — they live under `<ns>/user/` and materialize under `$HOME`:

```bash
cat > $AENV_HOME/envs/research/aenv.toml <<'EOF'
name = "research"
extends = []

[adapters.claude-code]
files = []
user_files = [
    ".claude/CLAUDE.md",
    ".claude/agents/explorer.md",
    ".claude/settings.json",
]
EOF
```

`aenv global list` finds it:

```bash
$BIN global list
```

```
research
```

`global list` filters to namespaces that declare at least one `user_files` entry — namespaces that exist only at project scope are silently skipped.

---

## Step 2 — activate it globally

```bash
$BIN global activate research
```

```
Activated 'research' globally in /tmp/aenv-home-XXXXXX
  + .claude/CLAUDE.md (Replace)
  + .claude/agents/explorer.md (Replace)
  + .claude/settings.json (Replace)
Note: running harness sessions retain their previous config until restart.
```

What lives under `~/.claude/` now:

```bash
ls -la $HOME/.claude/
```

```
drwxr-xr-x  .
drwxr-xr-x  ..
drwxr-xr-x  agents
lrwxrwxrwx  CLAUDE.md     -> /tmp/aenv-global-XXXXXX/.aenv/envs/research/user/.claude/CLAUDE.md
lrwxrwxrwx  settings.json -> /tmp/aenv-global-XXXXXX/.aenv/envs/research/user/.claude/settings.json
```

Each managed file is a symlink back into the namespace tree — editing the file under `$AENV_HOME/envs/research/user/` is the same as editing the live `~/.claude/` view. The activation state lives at `$AENV_HOME/global-state.json`:

```bash
$BIN global status
```

```
Active global namespace: research
Target root: /tmp/aenv-home-XXXXXX
Managed files: 3
  ~/.claude/CLAUDE.md
  ~/.claude/agents/explorer.md
  ~/.claude/settings.json
Note: running harness sessions retain their previous config until restart.
```

`aenv global which` answers "which namespace owns this file?":

```bash
$BIN global which ~/.claude/CLAUDE.md
```

```
~/.claude/CLAUDE.md -> research::.claude/CLAUDE.md
```

(The same path works as a relative `.claude/CLAUDE.md` argument too; `which` normalizes a leading `~/` or `/` before the lookup.)

> **What about an existing `~/.claude/settings.json`?** If the file already exists on disk before activation, `aenv` **backs it up** (moves it into a timestamped subdir of `$AENV_HOME/global-stash/`) and replaces it with the namespace's version. The original is restored byte-for-byte on `aenv global deactivate`. Today, global activation uses the same back-up-and-replace semantics as `aenv activate` — there is no deep-merge of the user's pre-existing settings.json with the namespace's snippet. The adapter declares `[user_default_merge] "~/.claude/settings.json" = "deep"`, but that strategy only fires when **two namespaces in the resolution chain** both contribute a `settings.json`; a single contributor always symlinks.

---

## Step 3 — swap to a `default` namespace in one transaction

Create a second namespace with a different `~/.claude/CLAUDE.md`:

```bash
$BIN create default --adapter claude-code
mkdir -p $AENV_HOME/envs/default/user/.claude
cat > $AENV_HOME/envs/default/user/.claude/CLAUDE.md <<'EOF'
# Default profile
Standard operating mode.
EOF
cat > $AENV_HOME/envs/default/aenv.toml <<'EOF'
name = "default"
extends = []

[adapters.claude-code]
files = []
user_files = [".claude/CLAUDE.md"]
EOF
```

Now activate `default` directly — there's no need to deactivate `research` first:

```bash
$BIN global activate default
```

```
Activated 'default' globally in /tmp/aenv-home-XXXXXX
  + .claude/CLAUDE.md (Replace)
Note: running harness sessions retain their previous config until restart.
```

Behind the scenes that one call performed two operations under a single global lock (`$AENV_HOME/global.lock`):

1. **Deactivated `research`** — removed every symlink it created, restored any backed-up originals (none in this run).
2. **Activated `default`** — backed up the new collisions, materialized `~/.claude/CLAUDE.md`, wrote a fresh `global-state.json`.

If step 2 had failed, `aenv` would re-activate `research` as best-effort rollback before returning the error. One activation lives per user, full stop.

Verify:

```bash
$BIN global status
```

```
Active global namespace: default
Target root: /tmp/aenv-home-XXXXXX
Managed files: 1
  ~/.claude/CLAUDE.md
Note: running harness sessions retain their previous config until restart.
```

```bash
cat $HOME/.claude/CLAUDE.md
```

```
# Default profile
Standard operating mode.
```

The `agents/explorer.md` and `settings.json` symlinks from the previous activation are gone — they belonged to `research` and were removed during its deactivate half of the transaction.

---

## Step 4 — `aenv global diff` between namespaces

A structural user-scope diff between two namespaces (no need to activate either):

```bash
$BIN global diff research default
```

```
User-scope diff 'research' vs 'default':
  -.claude/agents/explorer.md
  -.claude/settings.json
  ~.claude/CLAUDE.md
```

`-` is "in `research` but not in `default`"; `~` is "in both but with different bytes". The `--json` form is available for tooling consumption.

With no arguments and an active global namespace, `aenv global diff` is in *drift* mode — it compares the `$HOME` view against the namespace source:

```bash
$BIN global diff
```

```
No drift detected. Active global namespace matches its source.
```

---

## Step 5 — `aenv global doctor` catches an oversize CLAUDE.md

The built-in `claude-code` adapter declares `[user_soft_limits] instructions = 5000`. That triggers an automatic advisory `instructions_max_chars = 5000` check against every user-scope file with `role = "instructions"` (which, for `claude-code`, is `~/.claude/CLAUDE.md`).

Make a deliberately chatty namespace:

```bash
$BIN create chatty --adapter claude-code
mkdir -p $AENV_HOME/envs/chatty/user/.claude
python3 -c "print('# Chatty mode\n' + ('lorem ipsum dolor sit amet ' * 250))" \
  > $AENV_HOME/envs/chatty/user/.claude/CLAUDE.md
cat > $AENV_HOME/envs/chatty/aenv.toml <<'EOF'
name = "chatty"
extends = []

[adapters.claude-code]
files = []
user_files = [".claude/CLAUDE.md"]
EOF

wc -c $AENV_HOME/envs/chatty/user/.claude/CLAUDE.md
# → 6765 .../chatty/user/.claude/CLAUDE.md
```

Run the doctor against that namespace (no activation needed — pass the namespace name explicitly):

```bash
$BIN global doctor chatty
```

```
[WARN] instructions_max_chars chatty::~/.claude/CLAUDE.md — .claude/CLAUDE.md has 6765 chars (budget 5000). Refactor procedural content into skills, dispositional content into subagents, or use @-imports.
```

The `~/` prefix on the qualified-name target (`chatty::~/.claude/CLAUDE.md`) is the diagnostic marker that the outcome is user-scope rather than project-scope. The check is advisory at adapter defaults (`[WARN]`, exit 0). To make it blocking, declare the policy with `enforce = true` in the namespace manifest:

```toml
[policies]
instructions_max_chars = { value = 5000, enforce = true }
```

…and the next `aenv global activate chatty` will refuse (exit 17) before materializing anything.

---

## Step 6 — orphan-stash detection and `--prune`

Each global activation stores its backed-up originals under `$AENV_HOME/global-stash/<timestamp>/`. A clean deactivate consumes its own stash. If the process is killed mid-deactivate (or someone hand-deletes `global-state.json`), the stash directory is left behind — recoverable, but useless on its own. `aenv global doctor` detects these.

Simulate an abandoned stash and a live activation:

```bash
mkdir -p $AENV_HOME/global-stash/epoch-99
echo 'stash junk' > $AENV_HOME/global-stash/epoch-99/orphaned

$BIN global activate research >/dev/null
```

Now ask the doctor about global state as a whole (no namespace argument — defaults to the active one):

```bash
$BIN global doctor; echo "exit: $?"
```

```
[PASS] instructions_max_chars research::~/.claude/CLAUDE.md
Orphan stashes:
  /tmp/aenv-global-XXXXXX/.aenv/global-stash/epoch-99
error: global conflict: 1 orphan stash found; run `aenv global deactivate --prune` to clear
exit: 19
```

Exit code 19 is `GlobalConflict` — it's a hard error when the user is auditing global state as a whole. (When you pass a namespace name explicitly, `doctor` lists the orphans informationally and still exits 0; the namespace audit is the foreground task, the orphan list is an FYI.)

Clear it:

```bash
$BIN global deactivate --prune
```

```
Deactivated 'research' globally.
Pruned 1 orphan stash.
```

`--prune` runs `aenv global deactivate` first, then removes every directory under `$AENV_HOME/global-stash/` not referenced by the active state — which now includes the just-deactivated activation's own (now orphan) stash. Re-running `aenv global doctor` against an empty global state succeeds:

```bash
$BIN global doctor; echo "exit: $?"
```

```
error: activation conflict: no global activation; pass a namespace name to evaluate one directly
exit: 13
```

That's the no-activation form — pass a namespace name to evaluate one directly:

```bash
$BIN global doctor research
```

```
[PASS] instructions_max_chars research::~/.claude/CLAUDE.md
```

---

## Step 7 — the sugar form: `aenv use <ns> --global`

When you want both a project pin AND a global activation in one call:

```bash
mkdir -p /tmp/aenv-walk-proj && cd /tmp/aenv-walk-proj
$BIN use research --global
```

```
Pinned /tmp/aenv-walk-proj to namespace 'research'
Activated 'research' in /tmp/aenv-walk-proj
  + CLAUDE.md (Replace)
Activated 'research' globally in /tmp/aenv-home-XXXXXX
  + .claude/CLAUDE.md (Replace)
  + .claude/agents/explorer.md (Replace)
  + .claude/settings.json (Replace)
Note: running harness sessions retain their previous config until restart.
```

`aenv use <ns> --global` is exactly `aenv use <ns> && aenv activate && aenv global activate <ns>` — it writes the `.aenv` pin **and** materializes both surfaces. The whole namespace lands in one command.

Tear it down:

```bash
$BIN global deactivate
cd - >/dev/null
```

---

## Step 8 — wrap-up

A clean `aenv global deactivate` restores the original `~/.claude/` contents (every backed-up file moved back into place) and deletes `$AENV_HOME/global-state.json`:

```bash
$BIN global status
```

```
no global activation
```

Cleanup:

```bash
rm -rf "$AENV_HOME" "$HOME"
```

One more time, since it's the most common surprise: **running harness sessions retain their previous config until restart.** `aenv global` swaps files on disk; running Claude Code / Codex / Cursor processes read their config at startup and don't poll for changes. Quit and relaunch the harness when you want a swap to take effect.

---

## Reference appendix — paths aenv touches

Everything aenv reads or writes for the global scope lives under one of two roots:

- **State.** `$AENV_HOME/global-state.json` — JSON schema v5. Records the active namespace name, the target root (`$HOME`), every managed `<rel-path>`, the original location of every backed-up file. One per user; exists only while a global activation is live.
- **Stash.** `$AENV_HOME/global-stash/<timestamp>/<rel-path>` — pre-activation originals, moved here by `mv` so the restore is byte-perfect. `<timestamp>` is nanoseconds-since-epoch, so concurrent activations don't collide. Each activation has its own subdir; a clean deactivate consumes its own stash directory.
- **Lock.** `$AENV_HOME/global.lock` — advisory file lock acquired around any state-mutating global op. Concurrent `aenv global activate` invocations serialize cleanly; in-flight reads (`global status`, `global which`, `global list`) don't take the lock.
- **Materialized files.** `$HOME/<rel-path>` — what the agent harness sees. Symlinks back into the namespace source for single-contributor files (the common case at user scope).
- **Source layout.** `$AENV_HOME/envs/<ns>/user/<rel-path>` — what you hand-author. The `user/` subdir mirrors the materialization target one-to-one: a file at `~/.aenv/envs/research/user/.claude/CLAUDE.md` materializes at `$HOME/.claude/CLAUDE.md`.

A namespace's `aenv.toml` declares user-scope ownership via per-adapter `user_files = [...]`. Paths in `user_files` are written **without** the `~/` prefix (that prefix is reserved for the adapter manifest's own `user_files` declaration, which describes the surface in the abstract; the namespace lists concrete paths under that surface).
