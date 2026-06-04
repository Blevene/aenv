# `aenv-namespace.toml` Convention File Spec

> Status: stable since v0.1.0. Shipped in Milestone J Task 5 of the global-namespaces follow-ups.

## Purpose

A repository or directory that's intended to be imported as an aenv namespace
MAY ship an `aenv-namespace.toml` at its root. When `aenv global import
<source>` — or the one-command `aenv global use <source>`, which imports and
then activates — runs, this file (if present) is authoritative: it tells the
importer which adapters the source touches, where to place each piece in the
destination namespace, and which lifecycle scripts to wire up.

When `aenv-namespace.toml` is absent, the importer falls back to a heuristic
that recognizes a few well-known config layouts (notably `claude-ctrl`-style
repositories). The heuristic is documented in the import command's source and
is intentionally less expressive — authors of bespoke harness layouts should
ship a convention file.

**Lifecycle hooks are opt-in here only.** The heuristic imports config files
and never infers a lifecycle hook from a repo's `install.sh` (such scripts are
usually self-installers that conflict with aenv's materialization). If a
namespace needs a setup hook, declare it in the `[lifecycle]` block below; it
should do runtime-only work, since aenv has already placed the config.

## Schema

All fields are optional. An empty `aenv-namespace.toml` is legal (degenerate
but valid).

```toml
# Adapters this namespace touches. Each name must be installed as
# <aenv_home>/adapters/<name>.toml (or be a builtin) at activation time.
# The importer uses this only for documentation / validation today; the
# manifest's [adapters.<name>] sections are what activation actually reads.
adapters = ["claude-code", "codex"]

# Lifecycle hook scripts. Path values are relative to the namespace root
# (i.e. envs/<name>/), NOT to the source repo root, even though the importer
# reads them from the source. The importer copies the listed scripts in and
# the manifest stores the namespace-relative form.
[lifecycle]
on_activate = "install.sh"
on_deactivate = "uninstall.sh"

# Source-path -> target-path map. Keys are paths inside the source tree;
# values are paths inside envs/<name>/user/. A trailing "/" on either side
# means the entry is a directory; the importer copies recursively.
[layout]
"myrules/"        = ".claude/myrules/"
"CLAUDE.md"       = ".claude/CLAUDE.md"
"settings.json"   = ".claude/settings.json"
".codex/AGENTS.md" = ".codex/AGENTS.md"

# Source-relative paths to NOT copy. Useful for excluding docs, dev
# artifacts, screenshots. Supports a trailing "*" glob: "docs/*" matches
# every entry under docs/.
ignore = ["docs/", "*.tmp", "README.md"]
```

## Semantics

### `[layout]` table

For each `(source_path, target_path)` pair:

1. If `source_path` does not exist in the source tree, the entry is skipped
   silently. This lets the same convention file work for partial / optional
   subtrees.
2. If `source_path` matches an `ignore` entry, the entry is skipped.
3. Otherwise, the source is copied into `envs/<name>/user/<target_path>`.
   - Files copy as files.
   - Directories copy recursively.
4. The target path is recorded under whichever adapter's `[adapters.<name>]
   user_files` table corresponds to its prefix:
   - `.claude/...` -> `[adapters.claude-code]`
   - `.codex/...`  -> `[adapters.codex]`
   - anything else -> `[adapters.claude-code]` (fallback)

### `ignore` list

- Exact match: `"README.md"` skips `README.md` at the source root.
- Directory match: `"docs/"` skips `docs/` and everything under it.
- Suffix glob: `"*.tmp"` skips any path ending in `.tmp`.
- The `ignore` list applies before the layout map is consulted.

### `[lifecycle]`

The fields are round-tripped into the generated manifest's `[lifecycle]`
section verbatim. They are NOT executed at import time — execution lands in
Milestone K. For Task 5, the importer copies the referenced scripts into the
namespace dir (so `install.sh` ends up at `envs/<name>/install.sh`) and writes
the relative path into the manifest.

If `[lifecycle]` is omitted, no scripts are wired up.

### `adapters` list

Today this field is informational. The importer doesn't validate that every
target prefix in `layout` lands under an adapter in this list, but a future
release might. Authors should keep it accurate.

## Heuristic fallback

When the source tree contains no `aenv-namespace.toml`, the importer probes a
fixed set of source-relative paths and maps them as follows:

| Source path        | Target path under `user/`   |
|--------------------|------------------------------|
| `CLAUDE.md`        | `.claude/CLAUDE.md`          |
| `AGENTS.md`        | `.codex/AGENTS.md`           |
| `settings.json`    | `.claude/settings.json`      |
| `agents/`          | `.claude/agents/`            |
| `commands/`        | `.claude/commands/`          |
| `hooks/`           | `.claude/hooks/`             |
| `skills/`          | `.claude/skills/`            |
| `runtime/`         | `.claude/runtime/`           |
| `bin/`             | `.claude/bin/`               |
| `sidecars/`        | `.claude/sidecars/`          |
| `.codex/`          | `.codex/`                    |

The heuristic captures config only. It does **not** wire a repo's `install.sh`
/ `uninstall.sh` as lifecycle hooks — that is opt-in and must be declared in
the `[lifecycle]` block of an `aenv-namespace.toml` (see above). A bare
`install.sh` in the source is ignored by the heuristic.

Paths the heuristic doesn't know about are not captured. Authors who need
them must ship an `aenv-namespace.toml`.

## Round-trip example

Given a source tree:

```
~/work/claude-ctrl/
  CLAUDE.md
  agents/
    helper.md
  hooks/
    pre.sh
  install.sh
  uninstall.sh
```

Running `aenv global import ~/work/claude-ctrl my-ctrl` produces:

```
~/.aenv/envs/my-ctrl/
  aenv.toml
  install.sh
  uninstall.sh
  user/
    .claude/
      CLAUDE.md
      agents/helper.md
      hooks/pre.sh
```

and the manifest reads (canonical TOML):

```toml
name = "my-ctrl"

[adapters.claude-code]
user_files = [".claude/CLAUDE.md", ".claude/agents/", ".claude/hooks/"]

[lifecycle]
on_activate = "install.sh"
on_deactivate = "uninstall.sh"
```

The `[lifecycle]` section is appended as raw TOML text after the serde-
rendered body — the Rust `AenvManifest` struct doesn't have a `lifecycle`
field yet (it ships in Milestone K), but `from_toml` tolerates the unknown
section, so the round-trip is loss-free in practice.

## Importing for both scopes: `shared_files`

The importer always writes the captured paths under `user_files`, so a freshly
imported namespace is **global-only**: `aenv global use <ns>` (or `aenv activate
<ns> --global`) materializes it into `$HOME`, but it has nothing to materialize
into a project.

To reuse the same imported content **per-project as well**, without keeping a
second copy, promote the adapter block's `user_files` key to `shared_files`
after import (issue #5, Layer 2):

```toml
[adapters.claude-code]
shared_files = [".claude/CLAUDE.md", ".claude/agents/", ".claude/hooks/"]
```

No files move — the content stays under `envs/<name>/user/`. `shared_files`
entries are authored in the same user-scope layout as `user_files`; at activation
each one materializes to `$HOME/<rel>` under `--global` and to the project under
`--project`, from the single stored copy. A role-tagged file (the instructions
file) is remapped to each scope's own layout via the adapter's `roles` /
`user_roles` maps — e.g. `.claude/CLAUDE.md` lands at repo-root `CLAUDE.md` in a
project but `~/.claude/CLAUDE.md` globally — while non-role paths keep their
relative path in both scopes. The three buckets are: `files` (project only,
stored at the namespace root), `user_files` (user/global only, under `user/`),
and `shared_files` (both, under `user/`). Pass `--shared` to `aenv global import`
(or `aenv global snapshot` / `aenv global new` / `aenv create --global`) to emit
`shared_files` directly at import time, skipping the manifest edit.
