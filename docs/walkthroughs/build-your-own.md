# Walkthrough: build your own namespace from scratch

Goal: create a brand-new namespace, write a `CLAUDE.md`, author a skill, activate it in a project. About five commands end-to-end. Compact version lives in the [README's Creating-your-own-namespace section](../../README.md#creating-your-own-namespace).

> **New to aenv?** An **adapter** (e.g. `claude-code`) tells aenv which files a harness owns — here, `CLAUDE.md`. A **namespace** bundles those files; `aenv activate` *materializes* them into a project as symlinks. See the [glossary](./README.md#glossary).

## Prerequisites

- `aenv` installed and `~/.aenv/` populated — see [setup walkthrough](./setup-and-first-swap.md) if not.

## Step 1: Scaffold the namespace

```bash
aenv create my-style --adapter claude-code
```

Expected:
```
Created namespace 'my-style' at /home/you/.aenv/envs/my-style
```

Under `~/.aenv/envs/my-style/` you now have:

```
aenv.toml      # manifest with [adapters.claude-code] and files = ["CLAUDE.md"]
CLAUDE.md      # empty, ready to edit
```

`--adapter` scaffolds *both* the manifest declaration AND an empty version of every concrete file the adapter manages, so the namespace is immediately usable. No follow-up `aenv.toml` edit needed.

## Step 2: Write your `CLAUDE.md`

```bash
$EDITOR ~/.aenv/envs/my-style/CLAUDE.md
```

Anything you want. Example:

```markdown
# My personal working agreements

- Plan in 1-3 sentences before any non-trivial change.
- Tests in the same commit as the code they cover.
- Prefer deletion over abstraction.
```

## Step 3: Add a skill

```bash
aenv skill new commit-discipline --ns my-style
```

Expected:
```
Created authored skill 'commit-discipline' in namespace 'my-style':
  - /home/you/.aenv/envs/my-style/.claude/skills/commit-discipline/SKILL.md
  - registered in /home/you/.aenv/envs/my-style/aenv.toml
```

Under `~/.aenv/envs/my-style/`:

```
aenv.toml      # now also has a [[skills]] entry for commit-discipline
CLAUDE.md
.claude/
  skills/
    commit-discipline/
      SKILL.md   # scaffolded with name: / description: frontmatter
```

Fill in the skill:

```bash
$EDITOR ~/.aenv/envs/my-style/.claude/skills/commit-discipline/SKILL.md
```

The scaffold looks like:

```markdown
---
name: commit-discipline
description: TODO: describe this skill
---

# commit-discipline

Describe when the agent should invoke this skill.
```

Replace `TODO: describe…` with a "use when …" sentence — Claude Code uses the `description` to decide *when* to invoke your skill, so make it triggerable.

## Step 4: Pin and activate in a project

```bash
cd ~/code/some-project
aenv use my-style
aenv activate
```

Expected:
```
Pinned /home/you/code/some-project to namespace 'my-style'
Activated 'my-style' in /home/you/code/some-project
  + .claude/skills/commit-discipline/SKILL.md (Symlink)
  + CLAUDE.md (Symlink)
```

## Step 5: Confirm

```bash
aenv status
# Active namespace: my-style
# Resolution:       my-style
#
# Managed files:
#   ./.claude/skills/commit-discipline/SKILL.md
#       from my-style::.claude/skills/commit-discipline/SKILL.md
#   ./CLAUDE.md
#       from my-style::CLAUDE.md
#
# Skills (1 authored, 0 imported):
#   my-style::.claude/skills/commit-discipline/SKILL.md  authored  -
```

The `Resolution:` line shows the inheritance chain that produced this namespace — here just `my-style`, since it has no parent.

## Iterating

Both `CLAUDE.md` and the skill `SKILL.md` are symlinks pointing back at `~/.aenv/envs/my-style/`. Edits propagate immediately — no re-activate needed unless you *add* a new file to the namespace (in which case `aenv deactivate && aenv activate` picks it up — `deactivate` safely restores the project, `activate` re-materializes; see [setup-and-first-swap](./setup-and-first-swap.md)).

## Sharing across machines

`~/.aenv/envs/my-style/` is just a directory of files:

```bash
cd ~/.aenv/envs/my-style && git init && git add . && git commit -m "v1" && git push
```

Phase 6 will add first-class `aenv install`/`aenv sync` over git remotes. Until then, manual sync is the path.

## If something goes wrong

- **Undo this walkthrough:** `aenv deactivate` removes what aenv materialized and restores any originals it backed up; add `aenv unpin` to also drop the `.aenv` pin.
- **A step errored** (e.g. your project already had a `CLAUDE.md`)? aenv backs up displaced originals to `.aenv-state/backup/<timestamp>/` before writing. If `deactivate` didn't finish cleanly, `aenv restore` copies the latest backup back.

## What to read next

- [Install a skill from GitHub](./install-a-skill-from-github.md) — pull in third-party skills instead of authoring everything
- [Snapshot an existing project](./snapshot-an-existing-project.md) — start the other direction, from a project you've already shaped
- [Import a global profile from GitHub](./import-a-global-profile-from-github.md) — take a profile user-level (`~/.claude`) instead of per-project, and reuse one copy in both scopes (`shared_files`)
