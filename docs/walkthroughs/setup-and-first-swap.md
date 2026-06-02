# Walkthrough: install aenv and swap between starter namespaces

Goal: from a clean machine to actively swapping between the `karpathy` and `cherny` starter namespaces in your project, in about five minutes. Compact version of this flow lives in the [README's Installation + Try-the-built-in-namespaces sections](../../README.md#installation).

> **New to aenv?** A **namespace** is a named bundle of harness config (here, a `CLAUDE.md`). `aenv use` *pins* this project to one; `aenv activate` *materializes* it — symlinks its files into the project. See the [glossary](./README.md#glossary) for the rest.

## Prerequisites

- Rust 1.85+ via [rustup](https://rustup.rs)
- Git, an editor, a POSIX filesystem (Linux or macOS — Windows is Phase 7)

## Step 1: Install

```bash
git clone https://github.com/blevene/aenv
cd aenv
cargo install --path crates/aenv-cli --locked
```

`cargo install` compiles the `aenv-cli` package and drops the `aenv` binary into `~/.cargo/bin/`, which rustup already adds to `PATH`. If you used a different Rust install method, ensure `~/.cargo/bin` is on `PATH`:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc   # or ~/.bashrc
```

## Step 2: First-run bootstrap

The first `aenv` invocation creates `~/.aenv/` with built-in adapters and starter namespaces.

```bash
aenv --version
# → aenv 0.3.3

aenv list
# NAME                   EXTENDS                        ADAPTERS
# cherny                 -                              claude-code
# karpathy               -                              claude-code
```

Look at what got created:

```bash
ls ~/.aenv/envs/karpathy/
# → aenv.toml  CLAUDE.md
```

Both files are real text — `aenv.toml` is the manifest, `CLAUDE.md` is what'll end up in your project.

## Step 3: Pin a project to karpathy

Use any existing project directory here, or spin up a throwaway one with `mkdir -p ~/code/my-project && cd $_`.

```bash
cd ~/code/my-project
aenv use karpathy
```

Expected:
```
Pinned /home/you/code/my-project to namespace 'karpathy'
```

(`/home/you` is just the shell-expanded `~` — the same directory you `cd`'d into, printed in full.)

A one-line `.aenv` file lands at the project root containing `karpathy`. This records intent; nothing is materialized yet. Commit `.aenv` to git so collaborators know the project's expected namespace.

## Step 4: Activate

```bash
aenv activate
```

Expected:
```
Activated 'karpathy' in /home/you/code/my-project
  + CLAUDE.md (Symlink)
```

`CLAUDE.md` is now a symlink to `~/.aenv/envs/karpathy/CLAUDE.md`. Any pre-existing `CLAUDE.md` got moved into `.aenv-state/backup/<timestamp>/CLAUDE.md` and the move was recorded in `state.json`; see the [README §What happens to your existing files](../../README.md#what-happens-to-your-existing-files) for the safety guarantee.

## Step 5: Confirm

```bash
aenv status
# Active namespace: karpathy
# Resolution:       karpathy
#
# Managed files:
#   ./CLAUDE.md
#       from karpathy::CLAUDE.md

head -3 CLAUDE.md
# → ## 1. Think Before Coding
```

## Step 6: Swap to cherny

```bash
aenv deactivate
# Deactivated namespace 'karpathy' in /home/you/code/my-project

aenv use cherny
aenv activate
head -3 CLAUDE.md
# → ## Workflow Orchestration
```

That's the whole swap loop. Three commands per switch (`deactivate`, `use`, `activate`).

## Step 7 (when you're done)

```bash
aenv deactivate    # restores any pre-existing CLAUDE.md
aenv unpin         # drops the .aenv pin file (optional)
```

## If something goes wrong

- **Undo this walkthrough:** `aenv deactivate` removes the files aenv symlinked in and restores anything it backed up; add `aenv unpin` to also drop the `.aenv` pin.
- **`aenv activate` errored** (e.g. your project already had a `CLAUDE.md`)? aenv backs up displaced originals to `.aenv-state/backup/<timestamp>/` before writing. If `deactivate` didn't finish cleanly, `aenv restore` copies the latest backup back.

## What to read next

- [Build your own namespace from scratch](./build-your-own.md) — `aenv create --adapter` + author a skill
- [Install a skill from GitHub](./install-a-skill-from-github.md) — pull in a public skill, pinned for reproducibility
- [Snapshot an existing project](./snapshot-an-existing-project.md) — capture a hand-shaped `.claude/` tree as a reusable namespace
- [Import a global profile from GitHub](./import-a-global-profile-from-github.md) — swap your *user-level* `~/.claude` config; covers `--global` and one-copy-both-scopes (`shared_files`)
