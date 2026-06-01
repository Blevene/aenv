# Walkthrough: install a skill from a public GitHub repo

Goal: import one specific skill out of a public skill collection (the `scanpy` skill from [k-dense-ai/scientific-agent-skills](https://github.com/k-dense-ai/scientific-agent-skills)), pin it for reproducibility, and verify provenance after activation. Compact reference lives in the [README §Skills](../../README.md#skills).

> **New to aenv?** A **skill** is a reusable instruction module aenv adds to a **namespace**'s **manifest** (`aenv.toml`) and *materializes* into your project on `aenv activate`. This guide adds one from a git repo. New to namespaces? See [setup-and-first-swap](./setup-and-first-swap.md). Full [glossary](./README.md#glossary).

## Prerequisites

- `aenv` installed; a target namespace exists (`my-style` from [build-your-own](./build-your-own.md), or any other). Don't have one yet? Create a throwaway: `aenv create my-style --adapter claude-code` (or follow [build-your-own](./build-your-own.md)). Network access for the first activate.

## Step 1: Import the skill

```bash
aenv skill import git+https://github.com/k-dense-ai/scientific-agent-skills \
    --ns my-style \
    --path scientific-skills/scanpy \
    --pin v2.39.0
```

Expected:
```
Resolving git+https://github.com/k-dense-ai/scientific-agent-skills @ v2.39.0...
Imported skill 'scanpy' into namespace 'my-style':
  - source: git+https://github.com/k-dense-ai/scientific-agent-skills
  - scope: project
  - path: scientific-skills/scanpy
  - pinned ref: 9b13286d8bae8b87d0d1361bb945fd64de9817bc
  - registered in /home/you/.aenv/envs/my-style/aenv.toml
```

What happened:

- aenv parsed the source format (`git+<url>`).
- It derived the skill name from the path basename (`scanpy`).
- With `--pin v2.39.0`, aenv did a one-shot resolution to verify the tag exists and locked the resolved commit SHA into the manifest. Without `--pin`, the namespace would resolve to `HEAD` on each activation (great for tracking; bad for reproducibility).
- A `[[skills]]` entry was appended to `~/.aenv/envs/my-style/aenv.toml`. **No content has been fetched yet** — that's deferred until you activate.

Inspect the manifest entry:

```toml
[[skills]]
name = "scanpy"
mode = "imported"
adapter = "claude-code"
source = "git+https://github.com/k-dense-ai/scientific-agent-skills"
ref = "9b13286d8bae8b87d0d1361bb945fd64de9817bc"
path = "scientific-skills/scanpy"
required = false
```

## Step 2: Activate to fetch + materialize

```bash
cd ~/code/some-project
aenv use my-style
aenv activate
```

`aenv use` records the pin:
```
Pinned /home/you/code/some-project to namespace 'my-style'
```

Expected on first activation (the clone happens here):
```
Activated 'my-style' in /home/you/code/some-project
  + .claude/skills/scanpy/SKILL.md (Symlink)
  + .claude/skills/scanpy/assets/analysis_template.py (Symlink)
  + .claude/skills/scanpy/references/api_reference.md (Symlink)
  + .claude/skills/scanpy/references/plotting_guide.md (Symlink)
  + .claude/skills/scanpy/references/standard_workflow.md (Symlink)
  + .claude/skills/scanpy/scripts/qc_analysis.py (Symlink)
  + CLAUDE.md (Symlink)
```

aenv cloned the repo into `~/.aenv/cache/skills/<source-hash>/9b13286d.../` — and because you passed `--path`, it does a **sparse checkout of just `scientific-skills/scanpy/`** (not the whole monorepo), then symlinks that subtree's contents (SKILL.md + everything under it) into your project. Subsequent activations are network-free — pinned ref + cached.

## Step 3: Confirm provenance

```bash
aenv status
```

The Skills block at the bottom reports the resolved SHA per skill:

```
Skills (0 authored, 1 imported):
  my-style::.claude/skills/scanpy/SKILL.md  imported  git+https://github.com/k-dense-ai/scientific-agent-skills @ 9b13286d8bae8b87d0d1361bb945fd64de9817bc
```

The SHA is the same on every machine that activates with the same pin — that's the reproducibility guarantee.

You now have the `scanpy` skill symlinked into `.claude/skills/scanpy/`, pinned to an immutable commit that reproduces the same content on any machine.

## Step 4: Update to a new pin later

```bash
$EDITOR ~/.aenv/envs/my-style/aenv.toml
# change ref = "v2.40.0" (or any branch / tag / commit SHA)

aenv deactivate && aenv activate
# new ref → fresh cache entry under ~/.aenv/cache/skills/<hash>/<new-ref>/
# → fresh symlinks in your project
```

The old cache directory stays around (cheap; future `aenv cache prune` will clean it).

## Variations

**Single-skill repo (SKILL.md at the root):** drop `--path`:
```bash
aenv skill import git+https://github.com/example/some-skill --ns my-style --pin v1.0
```
aenv looks for `SKILL.md` at the cache root or under `<skill_name>/`.

**Local path** (testing a skill you're developing):
```bash
aenv skill import ~/code/skills/my-prototype --ns my-style
```
No clone, no pin — local sources resolve on every activation.

**Floating pin** (track `HEAD`):
```bash
aenv skill import git+https://github.com/example/skill --ns my-style
# no --pin → resolves to whatever HEAD points at on each activate
```
Trades reproducibility for staying current. State.json records the resolved SHA each time, so you can audit.

**Global profile (user scope):** project scope installs into this repo's `.claude/`; user scope installs into `~/.claude/` for every project. To add the skill to a profile you activate with `aenv global use` (so it installs into `~/.claude/skills/` rather than a project), pass `--scope user`:
```bash
aenv skill import git+https://github.com/k-dense-ai/scientific-agent-skills \
    --ns research --scope user --path scientific-skills/scanpy --pin v2.39.0
```
The `[[skills]]` entry gets `scope = "user"`, and the skill materializes at `~/.claude/skills/scanpy/` on `aenv global use research`. (Omit `--scope user` and the skill is project-scope and silently won't appear in a global activation.) See [updating a profile → Global profiles](./updating-a-profile.md#global-profiles-user-scope).

## What is `--pin` and where do I get the ref from?

**Required?** Optional. `aenv skill import <source> --ns <ns>` works without it — aenv just resolves to whatever the source's default branch HEAD points at on each activation. The recommendation is to *always* pin for anything you'll want to revisit, share with a teammate, or check into git, because without a pin you have no guarantee that two activations a week apart fetch the same content.

`--pin <ref>` locks the skill to a specific point in the source repo's history. Without it, aenv resolves to whatever the repo's default branch points at each time you activate — fine for casually trying a skill, but means two machines (or you, six months later) can see different content for the same namespace.

Three things you can pass to `--pin`:

| Form | What it is | Where on GitHub | Stability |
|---|---|---|---|
| **Tag** (e.g. `v1.0`, `v2.39.0`) | A human-named release marker | Repo page → **Releases** in the right sidebar, *or* click the branch dropdown (top-left) → **Tags** tab | Usually immutable; a maintainer *can* move a tag but it's a strong convention not to |
| **Commit SHA** (e.g. `07cbeeabd6022827c7ae88710af247472bd5d77e`) | The hex hash of a specific commit | Click any commit (Repo → **Commits**); the URL ends with the full SHA. The 7-char short form (e.g. `07cbeea`) works too. | Truly immutable — the SHA is computed from the commit's content |
| **Branch name** (e.g. `main`, `develop`) | A moving pointer | The branch dropdown at top-left of the repo page | Resolves to whatever HEAD of that branch is *at the moment you import*; the recorded `ref` in the manifest is the full SHA at that moment, so it doesn't auto-update on re-activation |

Regardless of which form you pass, aenv records the **full resolved SHA** in your manifest. So `--pin main` at noon and `--pin <sha-at-noon>` produce the same manifest — the difference is what happens if you later re-import (the branch form re-resolves; the SHA form is locked).

### How to read a SHA off a GitHub repo without tags

[`tasteray/skills`](https://github.com/tasteray/skills) is a real example of a repo with no releases or tags. To find a commit SHA:

1. Open the repo page.
2. Click **Commits** (under the branch dropdown, or at `https://github.com/<owner>/<repo>/commits/main`).
3. The top entry is the most recent commit. Click the short SHA on the right (~7 chars) — the URL changes to `…/commit/<full-sha>`. Copy the full SHA from the URL.
4. Alternatively, on any GitHub page press `y` to convert the URL to a permalink that includes the full SHA.

For [`k-dense-ai/scientific-agent-skills`](https://github.com/k-dense-ai/scientific-agent-skills), which *does* have releases:

1. Open the repo page → **Releases** in the right sidebar.
2. Pick a release (e.g. `v2.39.0`).
3. Pass `--pin v2.39.0` to aenv. (Tags read more cleanly in the manifest than 40-char SHAs.)

### Which form to use

- **Prefer a tag** if the repo has them. Semantic, stable, easy to read later. Tag names beat SHAs for code-review readability when you check in your namespace's `aenv.toml` to git.
- **Use a commit SHA** if there are no tags, or if you need a guarantee that a misbehaving maintainer can't move the version under you. SHAs are the only form that's *content-addressed* — they can't lie about what they point at.
- **Use a branch name** only when you actually want to track that branch's head, *and* you understand that aenv freezes the resolved SHA on import (re-import to pick up newer commits).

## If something goes wrong

- **Bad URL / a `--pin` ref that doesn't exist / `SKILL.md` not found under `--path`:** the import fails without modifying the namespace; fix the argument and re-run.
- **Undo the import:** `aenv skill remove <name> --ns <namespace>` removes the `[[skills]]` entry (and, for authored skills, the on-disk dir). For imported skills the cache clone stays under `~/.aenv/cache/skills/` — run `aenv cache prune` to reclaim it.
- **Undo materialization in a project:** `aenv deactivate` (add `aenv unpin` to also drop the `.aenv` pin).

## What to read next

- [Snapshot an existing project](./snapshot-an-existing-project.md) — the opposite direction: capture what's already there
- [Build your own namespace from scratch](./build-your-own.md) — if you want a fresh target for these imports
