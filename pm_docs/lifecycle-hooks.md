# Lifecycle hook contract

Authoritative spec for namespace authors writing `[lifecycle].on_activate` /
`[lifecycle].on_deactivate` scripts. This document defines the contract aenv
guarantees and the invariants authors MUST uphold.

If the code in `crates/aenv-core/src/activate/lifecycle.rs` ever disagrees
with this document, the code is the source of truth — open an issue.

## At a glance

| Field | Value |
|---|---|
| Manifest location | `aenv.toml` → `[lifecycle]` block |
| Recognised keys | `on_activate`, `on_deactivate` |
| Path semantics | Namespace-relative, no `..`, no leading `/` |
| Required hashbang | Yes — the script is `exec`'d directly |
| CWD when invoked | `$AENV_TARGET_ROOT` |
| Exit-status contract | `on_activate` failure → rollback + exit 19; `on_deactivate` failure → warn, continue |

## 1. Execution timing

### `on_activate`

Runs as the LAST step of `aenv global activate` (and `aenv use --global`),
AFTER all user-scope files have been materialized into `$HOME` and BEFORE
the global state file (`global-state.json`) is written. This ordering is
load-bearing: the script may probe the materialized surface (e.g.
`ls ~/.claude/`) but a failure must rewind every file we touched so the
user is left with the pre-activation state. State is not persisted until
the hook succeeds.

### `on_deactivate`

Runs as the FIRST step of `aenv global deactivate`, BEFORE any
materialized files are removed or originals restored. The script's view of
the world is "namespace still active." If you authored an `on_activate`
that, say, started a background process, this is your chance to stop it
while the materialized files it points at are still in place.

`on_deactivate` runs best-effort: a non-zero exit prints a warning and
file restoration continues. The deactivate transaction does not roll back.

## 2. Environment variables

Set by `run_lifecycle_script` for every lifecycle invocation:

| Variable | Value | Notes |
|---|---|---|
| `AENV_NAMESPACE` | Leaf namespace name (e.g. `"research"`) | Same as the `name` field in the namespace's manifest. |
| `AENV_SCOPE` | `"project"` or `"user"` | Today, `aenv global activate` always passes `"user"`; project-scope lifecycle is a future surface. |
| `AENV_TARGET_ROOT` | Absolute path to the activation root | `$HOME` for global activation; the project root for project activation. |
| `AENV_NAMESPACE_DIR` | Absolute path to the namespace dir | Typically `~/.aenv/envs/<ns>/`. The script's siblings (its `runtime/`, `templates/`, etc.) live here. |
| `AENV_LIFECYCLE_EVENT` | `"activate"` or `"deactivate"` | Useful when one script is symlinked to both keys. |
| `AENV_FORCE` | `"1"` if and only if the user passed `--force` to deactivate | Unset otherwise. Currently only meaningful for `on_deactivate`. |

Every other env var the user has is passed through unchanged (`$PATH`,
`$HOME`, etc.). The script inherits stdin/stdout/stderr from the parent
`aenv` process, so users see `pip install` progress, brew output, etc.
directly.

## 3. Working directory

`cwd` is `$AENV_TARGET_ROOT`. For global activation this is `$HOME`. If
your script needs to operate on files within the namespace (e.g. a
bundled `runtime/` you want to `pip install -e`), `cd
"$AENV_NAMESPACE_DIR/runtime"` first — don't assume `$PWD` points at the
namespace dir.

## 4. Exit codes

### `on_activate`

| Exit | aenv behavior |
|---|---|
| `0` | Activation succeeds. State file written. |
| Non-zero | Materialization is rolled back via the undo log. `aenv` returns `GlobalConflict` (exit code 19) with the message `on_activate failed: <error>; activation rolled back`. The user's `$HOME` looks untouched. No state file is written. |

### `on_deactivate`

| Exit | aenv behavior |
|---|---|
| `0` | Normal deactivation continues. |
| Non-zero | Warning printed to stderr (`warning: on_deactivate failed for '<ns>': <error>; continuing with file restoration`). File restoration runs anyway. Exit code 0 unless restoration itself fails. |

A missing `on_deactivate` script (declared in the manifest but the file
isn't on disk) prints a warning and continues — it does not abort the
deactivate.

## 5. Required author invariants

Namespace authors MUST:

- **Begin the script with a hashbang.** `#!/usr/bin/env bash`,
  `#!/usr/bin/env python3`, etc. The file is `exec`'d directly — no shell
  interpreter is implied.
- **Make the script executable.** `chmod +x install.sh`. aenv does not
  `chmod` the script before invoking it. (Note: if the file came from a
  git clone, the executable bit is preserved automatically.)
- **Be idempotent.** Running `on_activate` twice against the same target
  must be a no-op the second time. The user may activate, deactivate,
  re-activate; do not assume the world is empty when you start.
- **Be deterministic on failure.** If you fail partway through, exit
  non-zero and leave behind state that another invocation can clean up
  (or that's harmless to retry). aenv rolls back the files it managed,
  but anything your script touched (pip installs, brew packages,
  `~/Library/...`) is on you.
- **Use `set -euo pipefail` (or equivalent).** A silent partial failure
  that exits 0 is worse than a loud failure that triggers rollback.

Namespace authors MUST NOT:

- **Modify aenv state files.** Anything under `~/.aenv/state/`,
  `~/.aenv/global-state.json`, `~/.aenv/global-stash/`, or
  `~/.aenv/locks/` is off-limits. Touching these corrupts aenv's view
  of the world.
- **Remove materialized symlinks or files.** The files under
  `$AENV_TARGET_ROOT` that aenv materialized are owned by aenv until
  `deactivate`. Removing them strands aenv's undo log.
- **Spawn orphan daemons / background processes that aenv can't reap.**
  If you start a long-lived process in `on_activate`, kill it in
  `on_deactivate`. Failure to do so leaks resources past
  deactivation.
- **`rm -rf` anything outside files you created.** The user's `$HOME`
  contains data; don't be a wrecking ball.

## 6. Rollback semantics

aenv's rollback is **files-only**. When `on_activate` exits non-zero, the
undo log restores every file aenv created, replaced, or stashed — back
to the pre-activation state, byte-identical.

aenv does NOT undo side effects your script caused:

- Packages installed via `pip install --user`, `brew install`, `apt`, etc.
  remain installed.
- Files written outside `$AENV_TARGET_ROOT` (e.g. `~/Library/...` on
  macOS) remain.
- Network calls, database writes, etc. are not retracted.

If your `on_activate` causes side effects, your `on_deactivate` is the
ONLY place to undo them. Treat rollback as a "files restored, side
effects on you" boundary.

## 7. `aenv-rescue` does not run `on_deactivate`

(Forthcoming: see Task 14 — `aenv-rescue` is a recovery surface that
forcibly resets the user's `$HOME` from the global stash without
consulting namespace manifests.)

By design, `aenv-rescue` does NOT invoke `on_deactivate`. Rescue is the
"my activation is wedged, restore my files and forget the namespace"
escape hatch — it cannot trust a possibly-broken namespace's deactivate
script to behave. If your namespace requires cleanup beyond file
restoration, document that for users who reach for `aenv-rescue` so
they can do the cleanup manually.

If you want `on_deactivate` to run, use `aenv global deactivate`. If you
want it to run while still tolerating a failing script, use
`aenv global deactivate --force` (this skips `on_deactivate` but keeps
the rest of the deactivate transaction).

## 8. Approval model

The first time a namespace's `on_activate` is about to run, aenv prompts
the user for approval:

```
Namespace 'foo' wants to run an on_activate lifecycle script:
  <path to script>

Approve and remember this script's SHA-256? [y/N]
```

The approval is recorded at `~/.aenv/envs/<ns>/.approved` and pinned to
the script's SHA-256. Subsequent activations of the same namespace with
an UNCHANGED script proceed silently. If the script's contents change
(any byte), the SHA differs, the recorded approval is stale, and the
prompt re-fires.

`--yes` on `aenv global activate <ns>` (and on `aenv use <ns> --global`)
skips the prompt and records approval as if the user had answered "yes."
This is the right thing for non-interactive contexts (CI, scripted
provisioning); it is the WRONG thing if you don't trust the namespace
contents.

## See also

- `pm_docs/aenv-namespace-toml-spec.md` — full manifest grammar.
- `crates/aenv-core/src/activate/lifecycle.rs` — the implementation.
- `crates/aenv-core/src/manifest.rs::LifecycleHooks` — schema.
