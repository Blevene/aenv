# Walkthrough: snapshot an existing project into a reusable namespace

Goal: take a project whose `.claude/` tree and `CLAUDE.md` you've shaped by hand and capture it as a named namespace that you can reuse elsewhere. Compact reference lives in the [README §Capture an existing project](../../README.md#3-capture-an-existing-project).

## Prerequisites

- `aenv` installed; `~/.aenv/` populated (built-in adapters present).
- A project somewhere with hand-authored content matching one or more adapters' `files = [...]` patterns.

## The setup

For this walkthrough, assume the project at `~/code/the-shaped-project` looks like:

```
~/code/the-shaped-project/
├── CLAUDE.md                                 # hand-written working agreements
├── .claude/
│   ├── skills/linter-discipline/SKILL.md     # hand-authored skill
│   └── commands/review.md                    # slash command
└── .mcp.json                                 # MCP server config
```

Three files match the `claude-code` adapter (`CLAUDE.md` + everything under `.claude/`), one matches the `mcp` adapter (`.mcp.json`). All four are about to come along.

## Step 1: Snapshot

```bash
cd ~/code/the-shaped-project
aenv snapshot my-existing-style
```

Expected:
```
Snapshotted 4 files from '/home/you/code/the-shaped-project' into namespace 'my-existing-style'.
  claude-code: 3 files
  mcp: 1 file
```

The output groups counts by adapter — useful sanity check that nothing important was silently missed. If your project also has, say, a `.cursorrules`, you'd see `cursor: 1 file` too.

## Step 2: Inspect what got captured

```bash
find ~/.aenv/envs/my-existing-style -type f
```

```
~/.aenv/envs/my-existing-style/aenv.toml
~/.aenv/envs/my-existing-style/CLAUDE.md
~/.aenv/envs/my-existing-style/.claude/commands/review.md
~/.aenv/envs/my-existing-style/.claude/skills/linter-discipline/SKILL.md
~/.aenv/envs/my-existing-style/.mcp.json
```

The manifest declares every captured path explicitly (glob expansion is materialized into literals):

```toml
name = "my-existing-style"
extends = []

[adapters.claude-code]
files = [
    ".claude/commands/review.md",
    ".claude/skills/linter-discipline/SKILL.md",
    "CLAUDE.md",
]

[adapters.mcp]
files = [".mcp.json"]
```

## Step 3: Source project is untouched

`snapshot` is a one-way capture — it never writes a `.aenv` pin to the source project:

```bash
ls -la ~/code/the-shaped-project/.aenv 2>&1
# → No such file or directory
```

That means: you can keep working in the source project the way you always have, and the snapshot is a reusable copy living entirely under `~/.aenv/envs/my-existing-style/`. If you later want to activate the snapshot in the source project itself, `aenv use my-existing-style && aenv activate` from there — but `aenv activate` will detect the existing files and back them up to `.aenv-state/backup/<timestamp>/` before symlinking (see [README §What happens to your existing files](../../README.md#what-happens-to-your-existing-files)).

## Step 4: Reuse the captured namespace elsewhere

```bash
cd ~/other-project
aenv use my-existing-style
aenv activate
aenv status
```

All four files materialize at the documented paths:

```
Managed files:
  ./.claude/commands/review.md
      from my-existing-style::.claude/commands/review.md
  ./.claude/skills/linter-discipline/SKILL.md
      from my-existing-style::.claude/skills/linter-discipline/SKILL.md
  ./.mcp.json
      from my-existing-style::.mcp.json
  ./CLAUDE.md
      from my-existing-style::CLAUDE.md
```

## Step 5 (optional): Flip a captured skill to upstream-tracked

`snapshot` records every skill as `mode = "authored"` — the SKILL.md content lives in the namespace tree, self-contained. If `linter-discipline` actually has a public source you'd rather track, edit its `[[skills]]` block (you may need to add one if snapshot didn't generate it — snapshot writes adapter files but doesn't synthesize `[[skills]]` entries for what it captured):

```toml
[[skills]]
name = "linter-discipline"
mode = "imported"
adapter = "claude-code"
source = "git+https://github.com/yourorg/linter-discipline-skill"
ref = "v0.3.0"
```

Then remove the now-redundant captured copy:

```bash
rm -rf ~/.aenv/envs/my-existing-style/.claude/skills/linter-discipline/
```

On next `aenv deactivate && aenv activate`, aenv fetches from the git source instead of using the captured snapshot.

## Common follow-ups

- **Share across machines:** `cd ~/.aenv/envs/my-existing-style && git init && git push`. Namespace dirs are just files.
- **Diff against the source:** if you keep editing the source project, `aenv diff` (Phase 5) compares the materialized state against the namespace — useful to catch drift.
- **Combine with composition:** `aenv create new-style --extends my-existing-style` lets you build on top of the snapshot without touching the original.

## What to read next

- [Install a skill from GitHub](./install-a-skill-from-github.md) — for the imports the snapshot doesn't auto-create
- [Build your own namespace from scratch](./build-your-own.md) — the inverse path: start empty, add pieces
