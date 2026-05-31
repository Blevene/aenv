# aenv walkthroughs

Task-focused, copy-pasteable guides for getting things done with `aenv`. Each
one is a self-contained journey with the exact commands and the output you
should expect.

New here? Read the [glossary](#glossary) first (it's short), then follow the
walkthroughs in the recommended order below.

## Recommended order

**Start here**

1. [setup-and-first-swap](./setup-and-first-swap.md) — install, then swap
   between the two built-in namespaces (`karpathy` and `cherny`). The fastest
   way to see what aenv does.
2. [build-your-own](./build-your-own.md) — author your own namespace from
   scratch and activate it in a project.

**Then, as you need them**

- [snapshot-an-existing-project](./snapshot-an-existing-project.md) — capture a
  project's existing harness config into a reusable namespace.
- [install-a-skill-from-github](./install-a-skill-from-github.md) — add a skill
  from a git repo to a namespace, pinned to an immutable commit.
- [updating-a-profile](./updating-a-profile.md) — day-to-day edits to a
  namespace you already have (add a skill, bump a pin, change instructions).
- [shell-integration](./shell-integration.md) — auto-activate the right
  namespace as you `cd` between projects (convenience).

> Working with **global / user-scope** profiles (files under `~/.claude/`
> rather than a single project)? See [§Global namespaces in the
> README](../../README.md#global-namespaces).

## Glossary

The terms every walkthrough assumes. Definitions match `aenv --help`.

- **Namespace** — a named bundle of AI-harness config files (a `CLAUDE.md`,
  skills, agents, …) that aenv swaps in and out of a project or your home
  directory. Lives at `~/.aenv/envs/<name>/`.
- **Adapter** — aenv's definition of which files a given harness owns. The
  built-in `claude-code` adapter claims `CLAUDE.md` and `.claude/`. A namespace
  targets one or more adapters (`--adapter claude-code`).
- **Manifest** — the `aenv.toml` at a namespace's root. Declares its adapters,
  files, parameters, skills, and `extends`.
- **`use`** — pins the current project to a namespace by writing a small
  `.aenv` file at the project root. Does **not** place any files yet.
- **`activate` / materialize** — writes the namespace's files into the project
  as symlinks (or merged files where needed), backing up any displaced
  originals to `.aenv-state/backup/<timestamp>/`. "Materialize" = this step.
- **`deactivate`** — reverses `activate`: removes the files aenv materialized,
  restores backed-up originals byte-for-byte, and deletes `.aenv-state/`. Leaves
  the `.aenv` pin in place (`unpin` removes that too).
- **Pin (`.aenv`)** — the file `aenv use` drops at a project root marking which
  namespace belongs there. The shell hook reads it to auto-activate on `cd`.
- **`extends`** — inheritance. A namespace inherits its parent's files and
  overrides them section-by-section.
- **Scope** — *project* scope materializes into the current repo's `.claude/`;
  *global* (user) scope materializes into `~/.claude/` for every project, via
  the `aenv global …` commands.
- **resolved_hash** — a content hash of everything a namespace materializes.
  Identical inputs produce an identical hash, so you can verify two setups match
  across machines.

## If something goes wrong

Every project-scope walkthrough can be unwound with the same handful of
commands:

- **Undo a walkthrough:** `aenv deactivate` removes what aenv materialized and
  restores any originals it backed up; add `aenv unpin` to also remove the
  `.aenv` pin.
- **A step errored partway** (e.g. a pre-existing `CLAUDE.md`): aenv backs up
  displaced originals before writing. If `deactivate` didn't run cleanly,
  `aenv restore` copies the latest backup back into the project.
- **Remove a skill you added:** `aenv skill remove <name> --ns <namespace>`
  (then `aenv cache prune` to reclaim disk for imported skills).

Each walkthrough repeats the subset relevant to it.
