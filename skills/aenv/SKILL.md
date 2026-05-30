---
name: aenv
description: Use when the user wants to set up, switch between, or manage `aenv` namespaces — named bundles of CLAUDE.md, skills, MCP entries, and other AI-coding-harness config — in a project OR globally across `$HOME`. Triggers include `aenv …` mentioned directly, "switch namespace/profile", "activate/deactivate", "create/snapshot a namespace", "install a skill (from GitHub)", "auto-activate on cd", "what namespace am I using"; and for the global scope: "swap my global/user-level config (`~/.claude`, `~/.codex`)", "use a different Claude Code setup globally", "onboard a config repo like claude-ctrl", "swap back to my old setup". Don't use for secrets, Python venv, or Claude Code plugin marketplace ops.
---

# aenv

`aenv` is a CLI that manages named bundles of AI-coding-harness configuration (CLAUDE.md, .cursorrules, .mcp.json, skills, agents, etc.) — pin a project to a namespace, activate it, and the bundle materializes into the project as symlinks. Deactivate restores the project to its pre-activation state byte-for-byte.

## Mental model in three sentences

Namespaces live at `~/.aenv/envs/<name>/` and own a manifest (`aenv.toml`) plus the actual files. A project gets pinned to a namespace via a one-line `.aenv` file at its root; `aenv activate` reads the pin, walks the `extends` chain, and materializes the resolved bundle. State lives in `.aenv-state/state.json` and the displaced originals (if any) sit in `.aenv-state/backup/<timestamp>/` until `aenv deactivate` puts them back.

## Two scopes: project vs global

aenv works at two scopes, and the verb prefix tells them apart:

- **Project scope** (`aenv <verb>`) — materializes a namespace's project files into a pinned project directory (the section above). State under `<project>/.aenv-state/`.
- **Global scope** (`aenv global <verb>`) — materializes a namespace's *user-scope* files (declared as `user_files`) into `$HOME` (`~/.claude/`, `~/.codex/`, …). **One global activation lives per user**; activating another swaps it in a single transaction. State at `~/.aenv/global-state.json`, displaced originals stashed under `~/.aenv/global-stash/<ts>/`. This is how you swap your whole Claude Code / Codex setup. See the Global profiles section below.

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

## Global profiles (`aenv global` — user scope)

Swapping the user-level config that every Claude Code / Codex session reads. `aenv global use` is the one-command front door.

| User wants… | Run this |
|---|---|
| Onboard a config repo (e.g. claude-ctrl) and turn it on | `aenv global use <git-url\|local-path>` — imports it as a namespace (if not already present) **and** activates it, in one command. `--as <name>` to name it; `--pin <ref>` for a git ref |
| Switch to an already-installed global profile | `aenv global use <name>` |
| Swap back to the profile you were just on | `aenv global use -` (toggles to the previous) |
| Swap back to your original pre-aenv setup | `aenv global use baseline` (auto-captured on the first-ever global activation) |
| Author a global profile from scratch | `aenv global new <name>` (scaffolds an editable `~/.aenv/envs/<name>/user/.claude/CLAUDE.md` + manifest), then `aenv global use <name>` |
| Capture the current `~/.claude/` as a reusable namespace | `aenv global snapshot <name> [--include <path>…]` |
| Import a source without activating it | `aenv global import <source> [<name>] [--pin <ref>]` |
| What global profile is active / what it manages | `aenv global status` (`--json` for structured) |
| Which namespace owns `~/.claude/<file>` | `aenv global which <path>` (`--json` adds `content_hash`) |
| List global (user-scope) namespaces | `aenv global list` |
| Check a namespace's policies / clean orphan stashes | `aenv global doctor [<ns>] [--fix]` (`--fix` deletes orphan stash dirs) |
| Turn off the global profile (restore `$HOME`) | `aenv global deactivate` (add `--force` only if a broken `on_deactivate` hook blocks it) |
| Run the whole sequence non-interactively (CI) | append `--yes` to `use`/`activate` |

**Deprecated:** `aenv global activate <ns>` still works but prints a deprecation notice — prefer `aenv global use <ns>` (a superset that also imports sources and records a swap-back point).

## Global gotchas — surface these proactively

- **One activation per user.** `aenv global use <b>` while `<a>` is active deactivates `<a>` and activates `<b>` atomically; on failure `<a>` is restored. There's no "both at once."
- **First activation auto-captures `baseline`.** Your pre-aenv `~/` surface is saved as the `baseline` namespace so you always have a return point (`aenv global use baseline` / `use -`). `--no-baseline` opts out. An empty `$HOME` captures nothing.
- **Running sessions keep their config until restart.** aenv swaps files on disk; a live Claude Code / Codex process read its config at launch. Tell the user to quit and relaunch the harness for a swap to take effect — aenv prints this caveat on every activation.
- **Lifecycle hooks are opt-in, never inferred.** A namespace runs a setup script on activation only if it declares `[lifecycle] on_activate` in its manifest or a source repo's `aenv-namespace.toml`. aenv does NOT auto-run a repo's `install.sh` (those are self-installers that fight aenv). First run prompts for approval (SHA-pinned at `~/.aenv/envs/<ns>/.approved`); `--yes` skips the prompt.
- **Recovery / lockout.** If an activated namespace's hooks lock you out of a Claude Code session (e.g. a `settings.json` hook calls a missing runtime), open a **non-Claude** shell and run `aenv global deactivate --force`, or `aenv-rescue` (a standalone binary that restores `$HOME` from `global-state.json` via direct fs ops, never running lifecycle scripts).

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

For non-destructive ops (`aenv use`, `aenv activate`, `aenv deactivate`, `aenv status`, `aenv list`, and the `aenv global` verbs — `use`, `status`, `which`, `list`, `doctor`, `deactivate`) just run them. Global swaps are stash-backed and reversible (`aenv global use baseline` / `use -` / `deactivate` restore the prior state), so they don't need the destructive-op confirmation — the one caveat to mention is that a namespace's `on_activate` hook can have side effects aenv can't undo (it only restores the files it materialized).

## When the right answer isn't aenv

- The user wants **Claude Code plugins** (the official `/plugin marketplace add` / `/plugin install` system) — `aenv` doesn't manage those. They install globally into `~/.claude/plugins/` and are runtime-backed.
- The user wants **secrets management** (`.env`, API keys) — `aenv` is for harness *configuration*, not credentials. Point at a real secrets tool.
- The user wants **Python `venv` or Node `nvm`** — `aenv` doesn't manage language runtimes. Different problem.

## Where to look for more

- `aenv --help`, `aenv <subcommand> --help`, and `aenv global --help` for the full flag surface.
- `~/.aenv/envs/<ns>/aenv.toml` for the manifest of any namespace.
- `aenv status --json` / `aenv global status --json` for machine-readable everything.
- The repo's `pm_docs/` directory has step-by-step walkthroughs — notably `pm_docs/walkthrough-global-namespaces.md` (global profiles end-to-end) and `pm_docs/lifecycle-hooks.md` (the hook contract). (Plain paths, not links: this skill is imported standalone, so repo-relative links wouldn't resolve.)
