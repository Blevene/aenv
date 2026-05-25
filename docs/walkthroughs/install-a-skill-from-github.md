# Walkthrough: install a skill from a public GitHub repo

Goal: import one specific skill out of a public skill collection (the `scanpy` skill from [k-dense-ai/scientific-agent-skills](https://github.com/k-dense-ai/scientific-agent-skills)), pin it for reproducibility, and verify provenance after activation. Compact reference lives in the [README §Skills](../../README.md#skills).

## Prerequisites

- `aenv` installed; a target namespace exists (`my-style` from [build-your-own](./build-your-own.md), or any other). Network access for the first activate.

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

Expected on first activation (the clone happens here):
```
Pinned /home/you/code/some-project to namespace 'my-style'
Activated 'my-style' in /home/you/code/some-project
  + CLAUDE.md (Symlink)
  + .claude/skills/scanpy/SKILL.md (Symlink)
  + .claude/skills/scanpy/references/api_reference.md (Symlink)
  + .claude/skills/scanpy/references/standard_workflow.md (Symlink)
  + .claude/skills/scanpy/references/plotting_guide.md (Symlink)
  + .claude/skills/scanpy/scripts/qc_analysis.py (Symlink)
  + .claude/skills/scanpy/assets/analysis_template.py (Symlink)
```

aenv shallow-cloned the repo into `~/.aenv/cache/skills/<source-hash>/9b13286d.../`, then symlinked the contents of `scientific-skills/scanpy/` (SKILL.md + everything under it) into your project. Subsequent activations are network-free — pinned ref + cached.

## Step 3: Confirm provenance

```bash
aenv status
```

The Skills block at the bottom reports the resolved SHA per skill:

```
Skills (0 authored, 1 imported):
  my-style::.claude/skills/scanpy/SKILL.md  imported
      git+https://github.com/k-dense-ai/scientific-agent-skills @ 9b13286d8bae8b87d0d1361bb945fd64de9817bc
```

The SHA is the same on every machine that activates with the same pin — that's the reproducibility guarantee.

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

## What to read next

- [Snapshot an existing project](./snapshot-an-existing-project.md) — the opposite direction: capture what's already there
- [Build your own namespace from scratch](./build-your-own.md) — if you want a fresh target for these imports
