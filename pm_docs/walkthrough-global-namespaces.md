# Walkthrough: global namespaces with claude-cntrl

**Tested against:** `main`, `aenv 0.3.1`.
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
# → aenv 0.3.0
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

`global snapshot` walks every installed adapter's declared `user_files` against `$HOME` and copies what's there into a new namespace at `$AENV_HOME/envs/<name>/`. The set of captured paths follows adapter defaults; to add paths not declared by any installed adapter (say, a personal `~/.claude/scratch/`), pass `--include` repeatedly (snapshot into a fresh name — `default` already exists from the previous command, and snapshotting onto an existing namespace errors out):

```bash
$BIN global snapshot default-plus --include .claude/scratch
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

**(a) Import.** The importer prefers an `aenv-namespace.toml` at the source root if one is shipped (see [`aenv-namespace-toml-spec.md`](./aenv-namespace-toml-spec.md) for the convention file format); otherwise it falls back to a built-in heuristic that recognizes well-known config layouts (notably claude-ctrl-style repos). It picks up the config it knows — `CLAUDE.md`, `agents/`, `commands/`, `hooks/`, `skills/`, `settings.json`, `bin/`, `runtime/`, `sidecars/`, `.codex/`, … — and maps each under its adapter target. The generated `~/.aenv/envs/claude-cntrl/aenv.toml` reads roughly:

```toml
name = "claude-cntrl"
extends = []

[adapters.claude-code]
files = []
user_files = [
    ".claude/CLAUDE.md",
    ".claude/agents/",
    ".claude/bin/",
    ".claude/commands/",
    ".claude/hooks/",
    ".claude/runtime/",
    ".claude/settings.json",
    ".claude/sidecars/",
    ".claude/skills/",
]

[adapters.codex]
files = []
user_files = [".codex/", ".codex/AGENTS.md"]
```

Note there is **no `[lifecycle]` block**. The heuristic imports config files only — it does *not* auto-wire a repo's `install.sh` as `on_activate`. A repo's installer is typically a self-installer that wants to own `~/.claude` (validate a payload, back up your config, move itself into place); running it as an aenv hook would fight aenv's own materialization. If a namespace genuinely needs a runtime-setup hook, it declares one explicitly in `aenv-namespace.toml` (see [Lifecycle hooks](#lifecycle-hooks-opt-in) below). (`--pin <ref>` pins git URL sources to a tag or commit SHA for reproducibility; omit for `HEAD`. The standalone `aenv global import <source> [<name>]` does the import half only, without activating — useful when you want to inspect a namespace before turning it on.)

**(b) Baseline capture (first activation only).** Because this is the first global activation and no `baseline` namespace exists yet, aenv snapshots your current `~/` surface into `baseline` before materializing anything, then prints:

```
Captured your current ~/ surface as 'baseline' (swap back with: aenv global use baseline).
```

(If you ran `global snapshot default` in Step 1, or pass `--no-baseline`, this is skipped.)

**(c) Activate.** With no lifecycle hook wired, activation just materializes the config — no approval prompt. A pre-flight scan runs first and reports any `settings.json` hook / MCP / statusLine command paths that don't exist on disk (claude-ctrl references its own runtime scripts, which aren't installed in this bare example, so you'll see a batch of these); `--yes` proceeds past them without prompting:

```
Pre-flight found 33 potential issues:
  - hooks/PreToolUse in …/.claude/settings.json: command '$HOME/.claude/hooks/pre-bash.sh' references … (missing)
  …
Continuing because --yes was passed.
Captured your current ~/ surface as 'baseline' (swap back with: aenv global use baseline).
Activated 'claude-cntrl' globally in /tmp/aenv-home-XXXXXX
  + .claude/CLAUDE.md (Symlink)
  + .claude/agents (Symlink)
  + .claude/bin (Symlink)
  + .claude/commands (Symlink)
  + .claude/settings.json (Symlink)
  …
Backed up 1 file(s):
  - .claude/CLAUDE.md -> /tmp/aenv-global-XXXXXX/.aenv/global-stash/epoch-<ts>/.claude/CLAUDE.md
Note: running harness sessions retain their previous config until restart.
```

What happened:

1. `~/.claude/CLAUDE.md` already existed (you wrote it in Setup). It was moved to `$AENV_HOME/global-stash/<ts>/.claude/CLAUDE.md` for byte-perfect restore on deactivate.
2. Each managed path was symlinked back into `$AENV_HOME/envs/claude-cntrl/user/`.
3. The activation state file at `$AENV_HOME/global-state.json` was written — the activation is now persisted.

> **claude-ctrl's policy-engine runtime.** Because aenv didn't run claude-ctrl's `install.sh`, its Python runtime / `cc-policy` wiring isn't set up here — the pre-flight warnings above flag exactly that. The clean way to wire it is for claude-ctrl to ship an `aenv-namespace.toml` with a **runtime-only** `on_activate` (one that sets up the runtime, since aenv already placed the config). See [Lifecycle hooks](#lifecycle-hooks-opt-in).

Verify state:

```bash
$BIN global status
```

```
Active global namespace: claude-cntrl
Target root: /tmp/aenv-home-XXXXXX
Managed files: 11
  ~/.claude/CLAUDE.md
  ~/.claude/agents
  ~/.claude/bin
  …
Note: running harness sessions retain their previous config until restart.
```

`aenv global which` answers "which namespace owns this file?". Both the quoted `~/`-rooted form and a shell-expanded absolute path under `$HOME` resolve to the same managed file (the command strips the home prefix before matching), so either works:

```bash
$BIN global which '~/.claude/CLAUDE.md'
```

```
~/.claude/CLAUDE.md -> claude-cntrl::.claude/CLAUDE.md
```

With `--json`, the result also includes the file's `content_hash` — sha256 of the materialized bytes, useful for cross-machine verification. For a plain-file entry this is the sha256 of the file's bytes; for a directory-backed entry (like `claude-cntrl`'s `.claude/agents/`, which activation symlinks as a unit) `content_hash` is `null`, since a directory has no single content hash. Demonstrating against a single-file profile such as `default`:

```bash
$BIN global which '~/.claude/CLAUDE.md' --json
```

```json
{
  "content_hash": "sha256:...",
  "path": "~/.claude/CLAUDE.md",
  "qualified": "default::.claude/CLAUDE.md",
  "scope": "user",
  "strategy": "identical"
}
```

---

## Step 4 — use Claude Code with claude-cntrl active

This is where you actually do work. Open a fresh Claude Code session and the harness reads the now-materialized `~/.claude/CLAUDE.md`, the agents under `~/.claude/agents/`, and the hooks under `~/.claude/hooks/`. aenv's job is done — it's a file-mover, not a runtime.

Running harness sessions started **before** the activation keep their previous config until restart. `aenv global use` prints this caveat every time:

> Note: running harness sessions retain their previous config until restart.

Quit and relaunch the harness when you want a swap to take effect.

---

## Lifecycle hooks (opt-in)

A namespace can run a setup script on activation — but only if it **opts in**, by declaring `[lifecycle]` in its manifest (hand-authored) or in a source repo's `aenv-namespace.toml` (imported). aenv never infers a hook from a bare `install.sh`. A hook should do *runtime-only* setup (install a Python runtime, wire a binary onto `PATH`); aenv has already placed the config files, so a hook that re-copies config fights aenv.

Author a tiny opt-in example:

```bash
mkdir -p "$AENV_HOME/envs/hooked/user/.claude"
echo '# hooked profile' > "$AENV_HOME/envs/hooked/user/.claude/CLAUDE.md"
cat > "$AENV_HOME/envs/hooked/install.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "provisioning runtime for $AENV_NAMESPACE"
EOF
cat > "$AENV_HOME/envs/hooked/aenv.toml" <<'EOF'
name = "hooked"

[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "install.sh"
EOF

$BIN global use hooked
```

The first activation pauses and asks before running the script — showing its path, sha256, and first 8 lines:

```
About to run on_activate hook:
  Script: /tmp/aenv-global-XXXXXX/.aenv/envs/hooked/install.sh
  sha256: sha256:...
  First 8 lines:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "provisioning runtime for $AENV_NAMESPACE"
Allow this script to run on every future activation until its content changes? [y/N]:
```

Answer `y` and the script runs (you'll see `provisioning runtime for hooked`), then activation completes. The approval is namespace-scoped and SHA-pinned at `$AENV_HOME/envs/hooked/.approved`: subsequent activations with the same script content don't prompt; editing `install.sh` invalidates the approval and re-prompts. `--yes` records approval and skips the prompt (for CI / scripts). The aenv binary restores the script's executable bit before running it, so an imported script that lost its `+x` on copy still runs. Full contract — env vars, exit-code semantics, rollback — in [`lifecycle-hooks.md`](./lifecycle-hooks.md).

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

1. **Deactivated `claude-cntrl`** — removed every symlink it created, restored the backed-up `.claude/CLAUDE.md` from the stash, and deleted `claude-cntrl`'s state. (It declared no `on_deactivate`, so there was no lifecycle script to run; a namespace that opts into one would run it here first.)
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

## Step 5c — add skills to a global profile

A global profile can carry skills that install into `~/.claude/skills/` when you activate it. Use `aenv skill import` with **`--scope user`** — this is required for global profiles; without it the skill is project-scope and won't materialize on `aenv global use`.

Import a single skill from a monorepo subdir with `--path` (the import does a sparse checkout — it fetches only that subdir, not the whole repo):

```bash
$BIN skill import git+https://github.com/K-Dense-AI/scientific-agent-skills \
  --ns mine --scope user \
  --path skills/exploratory-data-analysis --pin main
```

```
Resolving git+https://github.com/K-Dense-AI/scientific-agent-skills @ main...
Imported skill 'exploratory-data-analysis' into namespace 'mine':
  - source: git+https://github.com/K-Dense-AI/scientific-agent-skills
  - scope: user
  - path: skills/exploratory-data-analysis
  - pinned ref: <resolved-sha>
  - registered in /tmp/aenv-home-XXXXXX/.aenv/envs/mine/aenv.toml
```

Notes:
- **`--adapter claude-code`** is needed only when the namespace declares more than one adapter (to disambiguate which one the skill belongs to); a single-adapter namespace infers it.
- `--scope user` writes `scope = "user"` on the `[[skills]]` entry. On `aenv global use mine`, the skill materializes at `~/.claude/skills/exploratory-data-analysis/` (symlinked from the cache). For an *authored* skill instead of an imported one, `aenv skill new <name> --ns mine --scope user` scaffolds it under the namespace's `user/.claude/skills/`.
- The `--pin <ref>` resolves to a commit SHA recorded in the manifest, so the skill set is reproducible across machines.

`aenv global which '~/.claude/skills/<name>/SKILL.md'` (once active, tilde quoted) reports which namespace owns it, and `aenv skill list --ns mine` shows every declared skill with its source, scope, and pinned ref.

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

(If you've already run a few activations, `doctor <name>` will also append an informational `Orphan stashes:` listing of any leftover stashes — passing a namespace name keeps that informational and still exits 0. See Step 7c for the no-argument form, which treats orphans as a hard error.)

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

(This clean output assumes no leftover stashes. The no-argument `global doctor` also audits global state and treats orphan stashes as a hard error (exit 19) — if earlier activations left stashes behind, you'll see an `Orphan stashes:` listing and a non-zero exit here. Clear them with `aenv global doctor --fix`; see Step 7c.)

This warning is the contract: under copy mode, the namespace source is the authoritative copy, and re-activation is destructive to local edits. If you want to preserve them, run `aenv global snapshot <new-name>` before the next activation. Under symlink mode (the default), this scenario doesn't arise — local edits are edits to the namespace source.

---

## Step 7 — when things go wrong: recovery

Three failure modes you might run into, in increasing order of severity.

### 7a — broken `on_deactivate`

`aenv global deactivate` runs the namespace's `on_deactivate` script first, then restores files. (`on_deactivate` only fires when the matching activation actually ran an `on_activate` hook — if there was no setup to undo, there's nothing to tear down. The heuristic-imported `claude-cntrl` declares no lifecycle hooks at all, so it never runs one.) To see this path, author a namespace whose activation runs a hook and whose teardown fails:

```bash
mkdir -p "$AENV_HOME/envs/brokenhook/user/.claude"
echo '# broken profile' > "$AENV_HOME/envs/brokenhook/user/.claude/CLAUDE.md"
printf '#!/usr/bin/env bash\necho "setup ok"\n' > "$AENV_HOME/envs/brokenhook/setup.sh"
printf '#!/usr/bin/env bash\nexit 1\n'          > "$AENV_HOME/envs/brokenhook/teardown.sh"
cat > "$AENV_HOME/envs/brokenhook/aenv.toml" <<'EOF'
name = "brokenhook"

[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "setup.sh"
on_deactivate = "teardown.sh"
EOF

$BIN global use brokenhook --yes >/dev/null
```

If the teardown script exits non-zero, aenv logs a warning and proceeds with file restoration anyway:

```bash
$BIN global deactivate
```

```
warning: on_deactivate failed for 'brokenhook': lifecycle script exited with exit status: 1; continuing with file restoration
Deactivated namespace 'brokenhook' globally in /tmp/aenv-home-XXXXXX
```

Exit code is 0 because file restoration succeeded. If the script itself is fundamentally broken (e.g. it depends on a runtime that's been uninstalled), skip it entirely with `--force` (re-activate `brokenhook` first if you want to try it):

```bash
$BIN global use brokenhook --yes >/dev/null
$BIN global deactivate --force
```

```
--force: skipping on_deactivate.
Deactivated namespace 'brokenhook' globally in /tmp/aenv-home-XXXXXX. (--force: skipped on_deactivate.)
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

See `pm_docs/walkthrough-global-namespaces.md` for the full recovery flow.
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
