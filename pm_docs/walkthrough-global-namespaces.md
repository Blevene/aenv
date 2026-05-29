# Walkthrough: global namespaces with claude-cntrl

**Tested against:** `main`, `aenv 0.1.0`.
**Goal:** onboard claude-ctrl from upstream in a single command (`aenv global use <url>`, which imports + activates and auto-captures a `baseline` return point), swap back to `baseline`, author your own profile from scratch with `aenv global new`, exercise the doctor surface, and walk through the full recovery story for when something breaks.

> **The short version.** Standing up an alternate global profile is one command:
> ```bash
> aenv global use https://github.com/juanandresgs/claude-ctrl
> ```
> That imports the repo as a namespace, captures your current `~/` as `baseline` (first time only), and activates it. Swap back with `aenv global use baseline`, or toggle to the profile you were just on with `aenv global use -`. The step-by-step below unpacks what that one command does.

A **global** namespace owns files under `$HOME` — `~/.claude/CLAUDE.md`, `~/.claude/agents/`, `~/.codex/AGENTS.md`, `~/.claude/settings.json`, …. The namespace's user-scope content lives at `~/.aenv/envs/<ns>/user/` and materializes under `$HOME` at activation time. One activation lives per user; activating a new namespace deactivates the prior one in a single transaction.

This walkthrough runs against an isolated `$HOME` so nothing touches your real `~/.claude/`.

For the project-scope walkthrough see [`walkthrough-three-harnesses.md`](./walkthrough-three-harnesses.md).

---

## Setup

Build the release binary:

```bash
cargo build --release
```

Pick scratch directories so the walkthrough never touches your real registry or your real `~/.claude/`:

```bash
export AENV_HOME=$(mktemp -d -t aenv-global-XXXXXX)/.aenv
export HOME=$(mktemp -d -t aenv-home-XXXXXX)
export BIN=$PWD/target/release/aenv
export RESCUE=$PWD/target/release/aenv-rescue

mkdir -p "$HOME"
$BIN --version
# → aenv 0.1.0
```

Two paths to keep in your head:

- **`$HOME`** — the surface global activations materialize into. `aenv global use <ns>` writes (symlinks or copies) files under here.
- **`$AENV_HOME`** — where the namespace registry lives, and also where the per-user **global activation state** is stored: `$AENV_HOME/global-state.json` (schema v6), stashes under `$AENV_HOME/global-stash/<timestamp>/`, the cross-process lock at `$AENV_HOME/global.lock`, and per-namespace lifecycle approval markers at `$AENV_HOME/envs/<ns>/.approved`.

Confirm the starting state:

```bash
$BIN global status
```

```
no global activation
```

Seed `$HOME` with a minimal `~/.claude/` so step 1 has something to capture:

```bash
mkdir -p "$HOME/.claude"
cat > "$HOME/.claude/CLAUDE.md" <<'EOF'
# My base profile
Standard operating mode.
EOF
```

---

## Step 1 — capture a return point (usually automatic)

You want a named namespace you can always switch back to. The first time you activate *any* global profile, aenv captures this for you automatically as a namespace called `baseline` — so you can skip ahead to Step 2 and a return point will exist after onboarding.

If you'd rather capture it explicitly (or name it yourself), `global snapshot` does that on demand:

```bash
$BIN global snapshot default
```

```
Snapshotted current ~/ user-scope surface into namespace 'default' (1 file, 0 directories captured).
  + .claude/CLAUDE.md
```

`global snapshot` walks every installed adapter's declared `user_files` against `$HOME` and copies what's there into a new namespace at `$AENV_HOME/envs/<name>/`. The set of captured paths follows adapter defaults; to add paths not declared by any installed adapter (say, a personal `~/.claude/scratch/`), pass `--include` repeatedly:

```bash
$BIN global snapshot default --include .claude/scratch
```

`default` is now a real namespace under `~/.aenv/envs/default/`:

```bash
cat $AENV_HOME/envs/default/aenv.toml
```

```toml
name = "default"
extends = []

[adapters.claude-code]
files = []
user_files = [".claude/CLAUDE.md"]
```

This is the point you can always swap back to. Re-activating `default` later is a byte-identical restore: the strategy reported on swap is `Identical` (no backup needed because the bytes already match).

---

## Step 2 — onboard claude-ctrl in one command

`aenv global use <target>` is the front door. When `<target>` is a git URL or local path, it imports the source as a namespace **and** activates it in a single step; when it's an existing namespace name it just switches to it; `-` toggles to the previous profile. Here we onboard claude-ctrl, naming the namespace `claude-cntrl` with `--as`:

```bash
$BIN global use https://github.com/juanandresgs/claude-ctrl --as claude-cntrl
```

aenv does three things under one command:

**(a) Import.** The importer prefers an `aenv-namespace.toml` at the source root if one is shipped (see [`aenv-namespace-toml-spec.md`](./aenv-namespace-toml-spec.md) for the convention file format); otherwise it falls back to a built-in heuristic that recognizes well-known layouts (notably claude-ctrl-style repos). Here the heuristic picks up `CLAUDE.md`, `agents/`, and `hooks/`, maps them under `.claude/`, and notices `install.sh` / `uninstall.sh` at the repo root and wires them as lifecycle hooks. The generated `~/.aenv/envs/claude-cntrl/aenv.toml` reads:

```toml
name = "claude-cntrl"
extends = []

[adapters.claude-code]
files = []
user_files = [
    ".claude/CLAUDE.md",
    ".claude/agents/",
    ".claude/hooks/",
]

[lifecycle]
on_activate = "install.sh"
on_deactivate = "uninstall.sh"
```

The `install.sh` itself is copied into the namespace dir root (NOT under `user/`) — at `~/.aenv/envs/claude-cntrl/install.sh`. Lifecycle scripts live alongside the namespace, not in its materialization surface. (`--pin <ref>` pins git URL sources to a tag or commit SHA for reproducibility; omit for `HEAD`. The standalone `aenv global import <source> [<name>]` does the import half only, without activating — useful when you want to inspect a namespace before turning it on.)

**(b) Baseline capture (first activation only).** Because this is the first global activation and no `baseline` namespace exists yet, aenv snapshots your current `~/` surface into `baseline` before materializing anything, then prints:

```
Captured your current ~/ surface as 'baseline' (swap back with: aenv global use baseline).
```

(If you ran `global snapshot default` in Step 1, or pass `--no-baseline`, this is skipped.)

**(c) Activate, with lifecycle approval.** The first time aenv is about to run an `on_activate` script for this namespace, it pauses and prints the script's full path, sha256, and first 8 lines, then asks for approval:

```
About to run on_activate hook:
  Script: /tmp/aenv-global-XXXXXX/.aenv/envs/claude-cntrl/install.sh
  sha256: sha256:df0271474a03150413c76ec1453b0cdc8acd9720aa6d717d551ac2632ec49b9f
  First 8 lines:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Running claude-cntrl install..."
    echo "  + provisioning policy engine"
    echo "  + ok"
Allow this script to run on every future activation until its content changes? [y/N]:
```

Read the script before you answer. `cat $AENV_HOME/envs/claude-cntrl/install.sh` shows it in full. Answer `y` to proceed:

```
Running claude-cntrl install...
  + provisioning policy engine
  + ok
Activated 'claude-cntrl' globally in /tmp/aenv-home-XXXXXX
  + .claude/CLAUDE.md (Symlink)
  + .claude/agents (Symlink)
  + .claude/hooks (Symlink)
Backed up 1 file(s):
  - .claude/CLAUDE.md -> /tmp/aenv-global-XXXXXX/.aenv/global-stash/epoch-<ts>/.claude/CLAUDE.md
Note: running harness sessions retain their previous config until restart.
```

What happened:

1. `~/.claude/CLAUDE.md` already existed (you wrote it in Setup). It was moved to `$AENV_HOME/global-stash/<ts>/.claude/CLAUDE.md` for byte-perfect restore on deactivate.
2. Each managed path was symlinked back into `$AENV_HOME/envs/claude-cntrl/user/`.
3. `install.sh` ran with `cwd = $HOME`, `AENV_NAMESPACE = "claude-cntrl"`, `AENV_TARGET_ROOT = $HOME`. Its stdout/stderr passed through directly.
4. The activation state file at `$AENV_HOME/global-state.json` was written — the activation is now persisted.
5. The approval was recorded at `$AENV_HOME/envs/claude-cntrl/.approved`, pinned to the script's sha256.

Subsequent activations of `claude-cntrl` with the same script content do NOT prompt — aenv compares the recorded sha against the current file. Editing `install.sh` invalidates the approval and re-prompts the next time around. This is the SHA-pinned approval contract from [`lifecycle-hooks.md` §8](./lifecycle-hooks.md#8-approval-model).

For non-interactive use, `--yes` proceeds without prompting — it records the lifecycle approval as if you'd answered yes, and proceeds past any pre-flight findings:

```bash
$BIN global use https://github.com/juanandresgs/claude-ctrl --as claude-cntrl --yes
```

The pre-flight scan still runs under `--yes` and prints what it found (e.g. a settings.json hook / MCP / statusLine command path that doesn't exist on disk yet because `install.sh` hasn't deposited its runtime) — `--yes` only suppresses the prompt, so you still see the warnings.

Verify state:

```bash
$BIN global status
```

```
Active global namespace: claude-cntrl
Target root: /tmp/aenv-home-XXXXXX
Managed files: 3
  ~/.claude/CLAUDE.md
  ~/.claude/agents
  ~/.claude/hooks
Note: running harness sessions retain their previous config until restart.
```

`aenv global which` answers "which namespace owns this file?":

```bash
$BIN global which ~/.claude/CLAUDE.md
```

```
~/.claude/CLAUDE.md -> claude-cntrl::.claude/CLAUDE.md
```

With `--json`, the result also includes the file's `content_hash` — sha256 of the materialized bytes, useful for cross-machine verification:

```bash
$BIN global which ~/.claude/CLAUDE.md --json
```

```json
{
  "content_hash": "sha256:...",
  "path": "~/.claude/CLAUDE.md",
  "qualified": "claude-cntrl::.claude/CLAUDE.md",
  "scope": "user",
  "strategy": "symlink"
}
```

---

## Step 4 — use Claude Code with claude-cntrl active

This is where you actually do work. Open a fresh Claude Code session and the harness reads the now-materialized `~/.claude/CLAUDE.md`, the agents under `~/.claude/agents/`, and the hooks under `~/.claude/hooks/`. aenv's job is done — it's a file-mover, not a runtime.

Running harness sessions started **before** the activation keep their previous config until restart. `aenv global use` prints this caveat every time:

> Note: running harness sessions retain their previous config until restart.

Quit and relaunch the harness when you want a swap to take effect.

---

## Step 5 — swap back to `default`

```bash
$BIN global use default
```

(Or `$BIN global use -` to toggle back to whatever profile you were on previously — aenv records the outgoing namespace on every swap. And `$BIN global use baseline` returns to the surface aenv auto-captured on your first activation.)

```
Activated 'default' globally in /tmp/aenv-home-XXXXXX
  + .claude/CLAUDE.md (Identical)
Note: running harness sessions retain their previous config until restart.
```

Behind the scenes that one call performed two operations under a single global lock (`$AENV_HOME/global.lock`):

1. **Deactivated `claude-cntrl`** — ran its `on_deactivate` script (`uninstall.sh` from the import, if it exists), removed every symlink it created, restored the backed-up `.claude/CLAUDE.md` from the stash, and deleted `claude-cntrl`'s state.
2. **Activated `default`** — applied `default`'s `user_files` against the now-restored `$HOME`. The strategy for `.claude/CLAUDE.md` is `Identical`: `default`'s source bytes already match what's at `~/.claude/CLAUDE.md` (because `default` is a snapshot of that file), so there's nothing to swap.

If step 2 had failed, aenv would re-activate `claude-cntrl` as best-effort rollback before returning the error. One activation lives per user, full stop.

```bash
cat $HOME/.claude/CLAUDE.md
```

```
# My base profile
Standard operating mode.
```

The `agents/` and `hooks/` symlinks from `claude-cntrl` are gone — they belonged to `claude-cntrl` and were removed during its deactivate half of the transaction. The `.claude/` directory now contains only what `default` declared.

---

## Step 5b — author your own profile from scratch

Importing an existing profile is one path; building your own is another. `aenv global new` scaffolds an editable user-scope namespace so you don't have to `mkdir user/.claude/` and hand-write the manifest:

```bash
$BIN global new mine
```

```
Created user-scope namespace 'mine' at /tmp/aenv-home-XXXXXX/.aenv/envs/mine
  + user/.claude/CLAUDE.md  (edit this, then run: aenv global use mine)
```

It seeds the adapter's instructions file (`~/.claude/CLAUDE.md` for the default `claude-code` adapter) under the namespace's `user/` subtree with a starter header, and pre-wires `[adapters.claude-code] user_files = [".claude/CLAUDE.md"]` in the manifest. Edit the file, then turn it on the same way as any other profile:

```bash
$EDITOR $AENV_HOME/envs/mine/user/.claude/CLAUDE.md
$BIN global use mine
```

`--adapter <name>` scaffolds for a different harness. This is the third way to create a namespace, alongside `global snapshot` (from your current `$HOME`) and `global import` / `global use <source>` (from an external tree).

---

## Step 6 — doctor your namespaces

`aenv global doctor` runs the user-scope policy evaluators. Three built-in checks fire today:

- **`instructions_max_chars`** — every user-scope file with `role = "instructions"` (which, for the claude-code adapter, is `~/.claude/CLAUDE.md`) is measured against the adapter's `[user_soft_limits] instructions` (5000 chars by default). Advisory `[WARN]` at the default; can be promoted to enforcing `[ERR]` (exit 17) per namespace.
- **`hook_paths_resolvable`** — every command path referenced by a settings.json `hooks` / `mcpServers` / `statusLine` entry is checked for existence on disk. Missing paths surface as warnings.
- **`copy_mode_local_edits`** — when a namespace uses `materialize = "copy"`, the doctor compares the materialized `$HOME` file against the namespace source. Local edits flag a warning that the next activation will overwrite them.

### 6a — oversize `~/.claude/CLAUDE.md`

Create a namespace with a deliberately chatty CLAUDE.md:

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

The `~/` prefix on the qualified-name target (`chatty::~/.claude/CLAUDE.md`) marks the diagnostic as user-scope rather than project-scope. The check is advisory at adapter defaults (`[WARN]`, exit 0). To make it blocking, add to the namespace manifest:

```toml
[policies]
instructions_max_chars = { value = 5000, enforce = true }
```

…and the next `aenv global use chatty` will refuse (exit 17) before materializing anything.

### 6b — copy-mode local edits

Author a namespace that opts into copy materialization:

```bash
$BIN create copyns --adapter claude-code
mkdir -p $AENV_HOME/envs/copyns/user/.claude
echo "# Copy mode profile" > $AENV_HOME/envs/copyns/user/.claude/CLAUDE.md
cat > $AENV_HOME/envs/copyns/aenv.toml <<'EOF'
name = "copyns"
extends = []

[adapters.claude-code]
files = []
user_files = [".claude/CLAUDE.md"]
materialize = "copy"
EOF

$BIN global deactivate >/dev/null
$BIN global use copyns
```

```
Activated 'copyns' globally in /tmp/aenv-home-XXXXXX
  + .claude/CLAUDE.md (Copy)
Backed up 1 file(s):
  - .claude/CLAUDE.md -> /tmp/aenv-global-XXXXXX/.aenv/global-stash/epoch-<ts>/.claude/CLAUDE.md
Note: running harness sessions retain their previous config until restart.
```

The strategy reads `(Copy)` instead of `(Symlink)`. Now edit the materialized file locally:

```bash
echo "# Edited locally" >> $HOME/.claude/CLAUDE.md
$BIN global doctor
```

```
[PASS] instructions_max_chars copyns::~/.claude/CLAUDE.md
[WARN] copy_mode_local_edits copyns::~/.claude/CLAUDE.md — ~/.claude/CLAUDE.md has been edited locally since activation; next activation will overwrite your edits. Run `aenv global snapshot <name>` first to capture.
```

This warning is the contract: under copy mode, the namespace source is the authoritative copy, and re-activation is destructive to local edits. If you want to preserve them, run `aenv global snapshot <new-name>` before the next activation. Under symlink mode (the default), this scenario doesn't arise — local edits are edits to the namespace source.

---

## Step 7 — when things go wrong: recovery

Three failure modes you might run into, in increasing order of severity.

### 7a — broken `on_deactivate`

`aenv global deactivate` runs the namespace's `on_deactivate` script first, then restores files. If the script exits non-zero, aenv logs a warning and proceeds with file restoration anyway:

```bash
$BIN global deactivate
```

```
Uninstalling claude-cntrl...
warning: on_deactivate failed for 'claude-cntrl': lifecycle script exited with exit status: 1; continuing with file restoration
Deactivated namespace 'claude-cntrl' globally in /tmp/aenv-home-XXXXXX
```

Exit code is 0 because file restoration succeeded. If the script itself is fundamentally broken (e.g. it depends on a runtime that's been uninstalled), skip it entirely with `--force`:

```bash
$BIN global deactivate --force
```

```
--force: skipping on_deactivate.
Deactivated namespace 'claude-cntrl' globally in /tmp/aenv-home-XXXXXX. (--force: skipped on_deactivate.)
```

File restoration still runs. `--force` sets `AENV_FORCE=1` in the lifecycle env (in case a script wants to behave differently under force) but, in practice, the script just doesn't run.

### 7b — locked-out Claude Code session

The risk pattern: claude-cntrl materializes a `~/.claude/settings.json` whose `hooks.PreToolUse` entry calls a runtime that wasn't installed (or got removed). Every Bash tool call inside an active Claude Code session fails-closed through that hook. You can't run `aenv global deactivate` from inside the Claude Code session because the hook blocks it.

Solution: open any **non-Claude** shell — a fresh terminal tab, the editor's terminal, an SSH session, anything that doesn't route through Claude Code's hook chain — and run:

```bash
aenv-rescue
```

```
Rescuing active global activation of 'claude-cntrl' under /tmp/aenv-home-XXXXXX
  removed symlink /tmp/aenv-home-XXXXXX/.claude/CLAUDE.md
  removed symlink /tmp/aenv-home-XXXXXX/.claude/agents
  removed symlink /tmp/aenv-home-XXXXXX/.claude/hooks
  restored /tmp/aenv-home-XXXXXX/.claude/CLAUDE.md

Rescue complete. Run `aenv global status` to confirm.
Note: aenv-rescue did NOT run the namespace's on_deactivate hook.
If the namespace's runtime needs uninstallation, do that manually.
```

`aenv-rescue` is a standalone binary that ships alongside `aenv`. It reads `$AENV_HOME/global-state.json` directly, walks the recorded managed files and backups, removes each materialized file, and restores each backup — using direct fs operations only. It NEVER spawns subprocesses, NEVER invokes `on_deactivate`, NEVER touches the Claude Code hook chain. It's panic-mode-friendly: nothing it does can go through the broken activation.

Two caveats:

1. **`aenv-rescue` does not invoke `on_deactivate`**, on purpose. Rescue is the "my activation is wedged, restore my files and forget the namespace" surface. If the namespace installed a runtime in `on_activate` that needs cleanup, do it by hand.
2. After rescue, the activation state file is gone and `$HOME` is back to its pre-activation surface. Investigate what broke (usually a `settings.json` referencing a missing executable) before reactivating.

The full rescue contract:

```bash
aenv-rescue --help
```

```
aenv-rescue — emergency deactivate for aenv global namespaces.

USAGE:
    aenv-rescue            Restore the user's $HOME from the active global
                           activation by reading $AENV_HOME/global-state.json
                           directly. Never spawns subprocesses; never runs the
                           namespace's on_deactivate hook; never touches the
                           Claude Code hook chain.

    aenv-rescue --help     Print this message.
    aenv-rescue --version  Print version and exit.

ENVIRONMENT:
    AENV_HOME              Override aenv home (default: $HOME/.aenv).
    HOME                   Required when AENV_HOME is unset.

EXIT CODES:
    0  Success (or no active activation; idempotent).
    1  State file present but unreadable or malformed.
    2  Unknown command-line argument.
```

### 7c — orphan stash

Each global activation writes a backed-up-originals dir under `$AENV_HOME/global-stash/<timestamp>/`. A clean deactivate consumes its own stash. If something went sideways mid-activation (process killed, hand-deleted `global-state.json`, two activations raced past the lock somehow), the stash directory survives but no state file points at it.

`aenv global doctor` (with no namespace argument) audits global state as a whole and flags orphans:

```bash
# Simulate an orphan stash:
mkdir -p $AENV_HOME/global-stash/epoch-99
echo 'orphan' > $AENV_HOME/global-stash/epoch-99/orphaned
$BIN global doctor; echo "exit: $?"
```

```
Orphan stashes:
  /tmp/aenv-global-XXXXXX/.aenv/global-stash/epoch-99
error: global conflict: 1 orphan stash found; run `aenv global doctor --fix` to clear
exit: 19
```

Exit code 19 is `GlobalConflict` — a hard error when the user is auditing global state. (Passing a namespace name to `doctor` reports orphans informationally and still exits 0; the namespace audit is the foreground task.)

Clear the orphan with `--fix`, which pairs the audit with its remediation:

```bash
$BIN global doctor --fix
```

```
Pruned 1 orphan stash.
```

`doctor --fix` deletes every orphan stash the audit finds — directories under `$AENV_HOME/global-stash/` not referenced by the active state — and then reports clean (exit 0). A live activation's own stash is referenced by state and is never touched. Re-running `aenv global doctor` against an empty global state now returns the no-activation form:

```bash
$BIN global doctor; echo "exit: $?"
```

```
error: activation conflict: no global activation; pass a namespace name to evaluate one directly
exit: 13
```

Exit 13 (`ActivationConflict`) — the doctor needs a target.

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

One more time, since it's the most common surprise: **running harness sessions retain their previous config until restart.** aenv global swaps files on disk; running Claude Code / Codex / Cursor processes read their config at startup and don't poll for changes. Quit and relaunch the harness when you want a swap to take effect.

---

## Reference appendix — paths aenv touches

Everything aenv reads or writes for the global scope lives under one of these roots:

- **State.** `$AENV_HOME/global-state.json` — JSON schema v6. Records the active namespace name, the target root (`$HOME`), every managed `<rel-path>` with its strategy, the original location of every backed-up file, `lifecycle_ran`, and `was_present_before_activation`. One per user; exists only while a global activation is live.
- **Stash.** `$AENV_HOME/global-stash/<timestamp>/<rel-path>` — pre-activation originals, moved here by `mv` so the restore is byte-perfect. `<timestamp>` is nanoseconds-since-epoch, so concurrent activations don't collide. A clean deactivate consumes its own stash.
- **Lock.** `$AENV_HOME/global.lock` — advisory file lock acquired around any state-mutating global op. Concurrent `aenv global use` invocations serialize cleanly; in-flight reads (`global status`, `global which`, `global list`) don't take the lock.
- **Lifecycle approval markers.** `$AENV_HOME/envs/<ns>/.approved` — file containing the sha256 of the `on_activate` script the user previously approved for `<ns>`. Subsequent activations with matching SHA proceed silently; mismatched SHA re-prompts.
- **Materialized files.** `$HOME/<rel-path>` — what the agent harness sees. Symlinks back into the namespace source under the default `materialize = "symlink"`; regular file copies under `materialize = "copy"`.
- **Source layout.** `$AENV_HOME/envs/<ns>/user/<rel-path>` — what you hand-author (or what the importer / snapshotter wrote). The `user/` subdir mirrors the materialization target one-to-one: a file at `~/.aenv/envs/claude-cntrl/user/.claude/CLAUDE.md` materializes at `$HOME/.claude/CLAUDE.md`.
- **Lifecycle scripts.** `$AENV_HOME/envs/<ns>/<script-name>.sh` — at the namespace dir root, NOT under `user/`. The manifest's `[lifecycle].on_activate` / `on_deactivate` values are namespace-relative paths to these files.

A namespace's `aenv.toml` declares user-scope ownership via per-adapter `user_files = [...]`. Paths in `user_files` are written **without** the `~/` prefix (that prefix is reserved for the adapter manifest's own `user_files` declaration, which describes the surface in the abstract; the namespace lists concrete paths under that surface).
