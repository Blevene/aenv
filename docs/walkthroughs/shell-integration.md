# Walkthrough: cd-based auto-activation with the shell hook

Goal: stop running `aenv use` / `aenv activate` / `aenv deactivate` by hand. After loading the shell hook, `cd`-ing into a project with a `.aenv` pin auto-activates it; `cd`-ing out auto-deactivates.

> **New to aenv?** A **pin** is the `.aenv` file `aenv use <ns>` drops at a project root to mark which namespace belongs there. The shell hook below reads it to *activate* the right namespace automatically as you `cd`. Full [glossary](./README.md#glossary).

## Prerequisites

- `aenv` installed (see [setup walkthrough](./setup-and-first-swap.md))
- At least one project pinned with `aenv use <namespace>` (see [build-your-own](./build-your-own.md))
- A shell that supports a chpwd hook: **bash**, **zsh**, or **fish**

## Step 1: Add the hook to your shell rc

Pick the snippet for your shell and add it to the rc file.

**zsh** — `~/.zshrc`:
```bash
eval "$(aenv init-shell zsh)"
```

**bash** — `~/.bashrc`:
```bash
eval "$(aenv init-shell bash)"
```

**fish** — `~/.config/fish/config.fish`:
```fish
aenv init-shell fish | source
```

Open a new shell (or `source` the rc file).

To confirm the hook loaded before you rely on it, run `type aenv` — it should succeed — or just `cd` into a pinned project and watch for the activation message the hook reports.

## Step 2: cd around — the hook does the rest

The example below uses placeholder paths like `/home/you/...` and project names like `my-project-a`; substitute your own pinned project paths. The comments just show what the hook reports.

```bash
cd ~/code/my-project-a
# _AENV_ACTIVE=/home/you/code/my-project-a (env var the hook tracks)
# ./CLAUDE.md now symlinks to the namespace pinned by my-project-a/.aenv

cd ~/code/my-project-b
# Hook deactivates A and activates B in one go.

cd ~
# No .aenv pin anywhere up the tree → hook deactivates B and clears _AENV_ACTIVE.

cd ~/code/my-project-a/subdir/deep
# Nearest-ancestor .aenv wins — re-activates A.
```

You can confirm what the hook thinks is active any time with:
```bash
echo "$_AENV_ACTIVE"
```

That's it — auto-activation is now live; the rest is optional background.

## What's actually running on each cd

The hook runs `aenv activate-if-needed "$_AENV_ACTIVE"` on every chpwd. That command:

1. Walks the cwd's ancestors looking for the nearest `.aenv` pin file.
2. Compares to the last value of `_AENV_ACTIVE` you passed in.
3. **Same project → no-op** (fast path; no state.json read or extends resolution).
4. **Different project (or first time entering one) →** deactivate the old project + activate the new one.
5. **Left every pinned scope →** deactivate the old project, return empty.
6. Prints the new project root (or empty) on stdout, which the hook captures into `_AENV_ACTIVE`.

The no-change path is just an ancestor walk + string compare — sub-millisecond on a warm cache.

## Coexistence with manual commands

You can still run `aenv use`/`activate`/`deactivate` by hand. Two caveats:

- **Manually activating a different namespace while the hook thinks you're in another:** the hook's `_AENV_ACTIVE` may briefly disagree with `state.json`. On the next `cd`, the hook recomputes from the pin, so things converge — but for the moment in between, `aenv status` is the source of truth, not `$_AENV_ACTIVE`.
- **Deleting `.aenv-state/` by hand while the hook thinks you're active:** the hook will try to deactivate something that's not there. It silently no-ops in that case. No corruption.

## Uninstalling the hook

Remove the `eval "$(aenv init-shell …)"` line from your shell rc and open a new shell. No registry state is touched — you can re-enable any time.

## If the hook isn't firing

- Open a **new** shell (or `source` your rc file) — the hook only loads at shell start.
- Confirm it loaded: `type aenv` should succeed and the hook function should be defined in the current shell.
- `cd` into a project that has a `.aenv` pin (run `aenv use <ns>` there first). With no pin in the current directory or any ancestor, the hook correctly does nothing.

## What to read next

- [Build your own namespace from scratch](./build-your-own.md) — if you don't have a pinned project yet
- [Updating an existing profile](./updating-a-profile.md) — the manual operations the hook complements
