---
name: aenv
description: Use when the user wants to set up, switch between, or manage `aenv` namespaces — named bundles of CLAUDE.md, skills, MCP entries, and other AI-coding-harness config. Triggers include `aenv …` mentioned directly, "switch namespace/profile", "activate/deactivate", "create/snapshot a namespace", "install a skill (from GitHub)", "auto-activate on cd", "what namespace am I using". Don't use for secrets, Python venv, or Claude Code plugin marketplace ops.
---

# aenv

`aenv` is a CLI that manages named bundles of AI-coding-harness configuration (CLAUDE.md, .cursorrules, .mcp.json, skills, agents, etc.) — pin a project to a namespace, activate it, and the bundle materializes into the project as symlinks. Deactivate restores the project to its pre-activation state byte-for-byte.

## Mental model in three sentences

Namespaces live at `~/.aenv/envs/<name>/` and own a manifest (`aenv.toml`) plus the actual files. A project gets pinned to a namespace via a one-line `.aenv` file at its root; `aenv activate` reads the pin, walks the `extends` chain, and materializes the resolved bundle. State lives in `.aenv-state/state.json` and the displaced originals (if any) sit in `.aenv-state/backup/<timestamp>/` until `aenv deactivate` puts them back.

## User request → command map

| User wants… | Run this |
|---|---|
| Set up a new namespace from scratch | `aenv create <name> --adapter claude-code` (also creates an empty `CLAUDE.md`; user edits it) |
| Build on top of an existing namespace | `aenv create <name> --extends <parent>` (composition; child overrides win section-by-section) |
| Capture a hand-shaped project as a reusable namespace | `aenv snapshot <name>` from inside the project |
| Pin + activate in a project | `aenv use <name>` then `aenv activate`. (Avoid `aenv activate <name>` without `use` first — it activates without writing a `.aenv` pin, leaving the project in a half-state where `aenv status` works but no future invocation knows what namespace was intended.) |
| Switch active namespace in a project | `aenv deactivate && aenv use <other> && aenv activate` |
| Restore project to pre-aenv state | `aenv deactivate` (then `aenv unpin` if they want the `.aenv` pin file gone too) |
| Add a skill they wrote themselves | `aenv skill new <skill-name> --ns <namespace>` |
| Install a skill from a public GitHub repo | `aenv skill import git+<url> --ns <ns> --pin <ref> [--path <subdir>]` — see "Pin selection" below |
| Remove a skill | `aenv skill remove <skill-name> --ns <ns>` |
| Reclaim cache space (orphaned skill clones) | `aenv cache prune` |
| See what's active in the current project | `aenv status` |
| See where a managed file came from | `aenv which <project-relative-path>` |
| Show the resolution chain + parameters + policies | `aenv status` (text) or `aenv status --json` (structured) |
| Compare two namespaces | `aenv diff <ns_a> <ns_b>` |
| Detect drift between project and active namespace | `aenv diff` (no args; runs against active) |
| Bump a pinned skill ref | edit `~/.aenv/envs/<ns>/aenv.toml`, change `ref =` line, then re-activate in any project using the namespace |
| Auto-activate on cd between projects | `aenv init-shell <bash\|zsh\|fish>` printed once into the user's rc file |

## Pin selection (for `aenv skill import --pin <ref>`)

When the user wants to import a skill, ask which form of pin they want (or pick a default and tell them):

- **Tag** (`v1.0`, `v2.39.0`) — when the source repo has releases. Semantic; reads cleanly. Find via repo's Releases tab.
- **Commit SHA** (40-char hex) — when there are no tags, or for absolute immutability. Find via repo's Commits view; the URL ends with the full SHA.
- **Branch name** (`main`) — only when the user actually wants to track that branch's head at the moment of import. aenv freezes the resolved SHA either way; re-import to follow main later.

`--pin` is optional. Without it, aenv re-resolves to the default branch HEAD on every activation. Always pin for anything reproducible.

## Gotchas — surface these proactively

- **Structural changes need a re-activate.** Adding/removing a managed file, adding/removing a skill, bumping a pinned ref → run `aenv deactivate && aenv activate` in every project where the namespace is active. Edits to *existing* files are live via symlink (no re-activate needed).
- **Editing through the symlink edits the namespace, not the project copy.** While a namespace is active, opening `CLAUDE.md` in an editor follows the symlink into `~/.aenv/envs/<ns>/CLAUDE.md`. The project's *original* (if any) sits in `.aenv-state/backup/<timestamp>/` and is restored on deactivate.
- **Don't delete `.aenv-state/` by hand while a namespace is active.** It's the backup-and-restore safety net. The directory disappears on its own after a clean deactivate. If they did delete it and need to recover, `aenv restore` is the escape hatch (copies the latest backup set back into the project).
- **`AENV_HOME` is the registry root.** Default `~/.aenv/`. The user can override per-invocation (`AENV_HOME=/path aenv …`) but everything in there is trusted — don't suggest pointing it at a shared location.

## Validating user intent before destructive ops

Before running these, confirm with the user:

- `aenv delete <ns>` — permanently removes a namespace from `~/.aenv/envs/`. Irreversible. Ask if they have a backup or if the namespace is pinned in any other project.
- `aenv fork` with no argument — detaches the *whole* project from its namespace by replacing every symlink with a real file copy. To re-attach, run `aenv activate` again — but the current local copies become the new backup, so any divergent edits the user made post-detach end up in `.aenv-state/backup/<timestamp>/` rather than as the live files. Ask whether they want to keep the detached copies or re-attach.
- `rm -rf ~/.aenv/envs/<ns>/` — manual, not aenv. Ask before running.

For non-destructive ops (`aenv use`, `aenv activate`, `aenv deactivate`, `aenv status`, `aenv list`, etc.) just run them.

## When the right answer isn't aenv

- The user wants **Claude Code plugins** (the official `/plugin marketplace add` / `/plugin install` system) — `aenv` doesn't manage those. They install globally into `~/.claude/plugins/` and are runtime-backed.
- The user wants **secrets management** (`.env`, API keys) — `aenv` is for harness *configuration*, not credentials. Point at a real secrets tool.
- The user wants **Python `venv` or Node `nvm`** — `aenv` doesn't manage language runtimes. Different problem.

## Where to look for more

- `aenv --help` and `aenv <subcommand> --help` for the full flag surface.
- `~/.aenv/envs/<ns>/aenv.toml` for the manifest of any namespace.
- `aenv status --json` for machine-readable everything.
- The repo's `docs/walkthroughs/` directory has step-by-step recipes for the common journeys.
