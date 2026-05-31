# Walkthrough: updating an existing profile

Goal: cover the full lifecycle of an existing namespace — adding, editing, and removing skills, files, and instructions — and when each change needs a re-activate to take effect.

> **New to aenv?** This guide edits a namespace you already have. A **namespace** is a bundle of harness config; its **manifest** is `aenv.toml`. After most edits you re-activate (`aenv deactivate && aenv activate`) to *materialize* the change into your project. If namespaces are new to you, start with [setup-and-first-swap](./setup-and-first-swap.md). Full [glossary](./README.md#glossary).

## Prerequisites

- A namespace you already created (e.g., `my-profile` from [build-your-own](./build-your-own.md))
- Optional: the namespace already activated in one or more projects

## The mental model

There are two kinds of change you can make to a namespace:

| Change | Lives via | Re-activate needed? |
|---|---|---|
| Edit an existing managed file's content | symlink | **No** — the symlink resolves to the namespace dir, so edits are visible immediately |
| Add a new file to the namespace | manifest + disk | **Yes** — `aenv activate` re-walks the manifest |
| Remove a file from the namespace | manifest + disk | **Yes** |
| Add a skill (authored or imported) | manifest + disk / cache | **Yes** |
| Remove a skill | manifest + disk | **Yes** |
| Bump an imported skill's pinned `ref` | manifest + new cache | **Yes** |
| Change parameters or policies (`[parameters]`, `[policies]`) | manifest only | No — read fresh by `aenv status` / `aenv doctor` |

"Re-activate" means: in every project where the namespace is currently active, run `aenv deactivate && aenv activate`. Until then, that project sees the previous state of the namespace.

## Operations × content type

|                                | Add                                 | Update                                                       | Remove                                      |
|--------------------------------|-------------------------------------|--------------------------------------------------------------|---------------------------------------------|
| **Skill — authored**           | `aenv skill new <name> --ns <ns>`   | Edit `~/.aenv/envs/<ns>/.claude/skills/<name>/SKILL.md`     | `aenv skill remove <name> --ns <ns>`        |
| **Skill — imported (git)**     | `aenv skill import git+… --pin <ref>` | Bump `ref =` in manifest, re-activate                      | `aenv skill remove <name> --ns <ns>` (cache stays; `aenv cache prune` reclaims) |
| **Instructions (CLAUDE.md etc.)** | Edit manifest `files`, create file | Edit the file in the namespace dir (or via the project symlink) | Edit manifest `files`, `rm` the file        |

Each row in detail below.

---

## Adding a skill

### Authored — new SKILL.md from scratch

```bash
aenv skill new commit-discipline --ns my-profile
$EDITOR ~/.aenv/envs/my-profile/.claude/skills/commit-discipline/SKILL.md
```

The CLI scaffolds the SKILL.md and appends a `[[skills]]` block to the manifest with `mode = "authored"`. In any active project: `aenv deactivate && aenv activate` to pick up the new file as a symlink in `.claude/skills/`.

### Imported — pull from a git repo

```bash
aenv skill import git+https://github.com/k-dense-ai/scientific-agent-skills \
    --ns my-profile \
    --path scientific-skills/scanpy \
    --pin v2.39.0
```

The CLI appends a `[[skills]]` block with `mode = "imported"` plus `source`, `ref`, and `path`. No content is fetched yet — that happens on the next `aenv activate`, which shallow-clones into `~/.aenv/cache/skills/<source-hash>/<ref>/` and symlinks into `.claude/skills/<skill_name>/`.

See the [full skill-import walkthrough](./install-a-skill-from-github.md) for the variations (no-pin, sub-path semantics, local paths).

---

## Updating a skill

### Editing its content (authored)

```bash
$EDITOR ~/.aenv/envs/my-profile/.claude/skills/commit-discipline/SKILL.md
```

Live via symlink — no re-activate needed. Any project where `my-profile` is active immediately sees the new content the next time the agent reads it.

### Bumping the pinned ref (imported)

```bash
$EDITOR ~/.aenv/envs/my-profile/aenv.toml
```

Find the `[[skills]]` block and change the `ref` line:

```toml
[[skills]]
name = "scanpy"
mode = "imported"
adapter = "claude-code"
source = "git+https://github.com/k-dense-ai/scientific-agent-skills"
ref = "v2.39.0"        # ← change this to v2.40.0, a branch name, or a commit SHA
path = "scientific-skills/scanpy"
```

A pin change is a structural change. Re-activate everywhere `my-profile` is in use:

```bash
cd ~/code/some-project
aenv deactivate && aenv activate
```

aenv fetches the new ref into a fresh cache directory; the old one stays around until you run `aenv cache prune` (which walks every namespace's `[[skills]]` entries and deletes any cache dir nothing references). The project's symlinks update to point at the new cache on the next activate.

### Changing source URL or `path`

Same flow as bumping the ref — manifest edit, then re-activate. Note that changing `source` produces a different cache hash entirely (new clone on first activation).

---

## Removing a skill

```bash
aenv skill remove commit-discipline --ns my-profile
```

For an authored skill this deletes the manifest's `[[skills]]` block AND the on-disk directory at `~/.aenv/envs/my-profile/.claude/skills/commit-discipline/`. For an imported skill it removes only the manifest block — the `~/.aenv/cache/skills/<hash>/<ref>/` clone is left in place so it can serve other namespaces using the same source+ref. Run `aenv cache prune` to reclaim any cache dirs that aren't referenced anywhere.

Either way, re-activate in every project where `my-profile` is in use to drop the now-stale symlink:

```bash
cd ~/code/some-project
aenv deactivate && aenv activate
```

---

## Adding instructions (a new managed file)

If you want, say, an MCP server config alongside your `CLAUDE.md`:

### Step 1: Declare the file in the manifest

Edit `~/.aenv/envs/my-profile/aenv.toml` and add an adapter block (if not already present) plus the file path:

```toml
[adapters.mcp]
files = [".mcp.json"]
```

### Step 2: Create the file in the namespace dir

```bash
echo '{"mcpServers": {}}' > ~/.aenv/envs/my-profile/.mcp.json
```

### Step 3: Re-activate

```bash
cd ~/code/some-project
aenv deactivate && aenv activate
```

The project now has `.mcp.json` symlinked alongside `CLAUDE.md`.

---

## Updating instructions

Edit the file in the namespace dir (or via the project's symlink — same file):

```bash
# Direct edit in the namespace dir:
$EDITOR ~/.aenv/envs/my-profile/CLAUDE.md

# Or via the symlink in any active project:
$EDITOR ~/code/some-project/CLAUDE.md
```

Live via symlink. No re-activate.

The only edits that need a re-activate are ones that *change which paths are managed* — adding a new section to `CLAUDE.md` doesn't qualify; adding `files = [..., ".claude/notes.md"]` to the manifest does.

---

## Removing instructions

Two steps:

```bash
# 1. Remove the path from `files = [...]` in the manifest
$EDITOR ~/.aenv/envs/my-profile/aenv.toml

# 2. Delete the file from the namespace dir
rm ~/.aenv/envs/my-profile/.mcp.json

# 3. Re-activate to drop the symlink in active projects
cd ~/code/some-project
aenv deactivate && aenv activate
```

If the project had its own `.mcp.json` *before* aenv first activated, that original file is in `.aenv-state/backup/<timestamp>/` and gets restored on `aenv deactivate` — but only as long as the namespace was still managing the path at the time. To be safe: deactivate first, then remove the file from the manifest. See [README §What happens to your existing files](../../README.md#what-happens-to-your-existing-files).

---

## A worked example: add a skill to an existing `my-profile`

```bash
# Today my-profile only has a CLAUDE.md.
aenv skill list --ns my-profile

# Pull in scanpy from k-dense-ai's monorepo, pinned for reproducibility.
aenv skill import git+https://github.com/k-dense-ai/scientific-agent-skills \
    --ns my-profile \
    --path scientific-skills/scanpy \
    --pin v2.39.0

# Manifest now has a [[skills]] entry; nothing fetched yet.
aenv skill list --ns my-profile     # → scanpy, mode = imported, pin = v2.39.0

# In a project that has my-profile active, re-activate to fetch + materialize.
cd ~/code/some-project
aenv deactivate && aenv activate    # clones into ~/.aenv/cache/skills/...
aenv status                          # confirms scanpy materialized with resolved SHA
```

That's the whole project-scope lifecycle: you can now add, edit, and remove skills, files, and instructions in a namespace, and you know which changes are live-via-symlink and which need a re-activate to materialize.

---

## Global profiles (user scope)

*Skip this section unless you manage global (user-scope) profiles — the project-scope flow above is complete on its own.*

Everything above is project scope. A **global** profile (one you activate with `aenv global use <ns>`, materializing into `~/.claude/` etc.) updates the same way, with two differences: skills and files must be declared **user-scope**, and the re-activate command is `aenv global use <ns>` (run it again; it re-materializes) rather than `aenv deactivate && aenv activate`.

### Add a skill to a global profile

Pass `--scope user` — without it the skill is project-scope and won't materialize on `aenv global use`:

```bash
aenv skill import git+https://github.com/k-dense-ai/scientific-agent-skills \
    --ns research --scope user \
    --path scientific-skills/scanpy --pin v2.39.0
# (add --adapter <name> only if the namespace declares more than one adapter)
```

This writes `scope = "user"` on the `[[skills]]` entry. On the next `aenv global use research`, the skill materializes at `~/.claude/skills/scanpy/`. For an authored skill instead: `aenv skill new <name> --ns research --scope user` (scaffolds under the namespace's `user/.claude/skills/`). The import does a sparse checkout of just `--path`, so one skill out of a big monorepo stays small.

### Add an arbitrary file to a global profile

To add any user-level file beyond what the adapter declares (e.g. `~/.claude/settings.json`, a `~/.claude/RTK.md`, a whole `~/.claude/statusline/` dir):

```bash
# 1. Place the content under the namespace's user/ subtree, mirroring its
#    target path under $HOME (e.g. ~/.claude/RTK.md -> user/.claude/RTK.md):
mkdir -p ~/.aenv/envs/research/user/.claude
cp ~/.claude/RTK.md ~/.aenv/envs/research/user/.claude/RTK.md

# 2. Declare the path in the manifest's user_files (under the right adapter):
$EDITOR ~/.aenv/envs/research/aenv.toml
#   [adapters.claude-code]
#   user_files = [".claude/CLAUDE.md", ".claude/RTK.md"]   # ← add it
#   (a trailing "/" like ".claude/statusline/" declares a whole directory)

# 3. Re-activate the profile:
aenv global use research
```

`user_files` is not capped by what the adapter declares — any relative path that doesn't escape with `..` works. `aenv global doctor research` flags issues (e.g. a `settings.json` whose hook commands point at scripts you didn't include).

## If something goes wrong

- **A re-activate failed or left the project in a weird state:** `aenv deactivate` restores backed-up originals and clears `.aenv-state/`; `aenv restore` copies the latest `.aenv-state/backup/<timestamp>/` back if deactivate didn't finish cleanly.
- **Undo a skill you added:** `aenv skill remove <name> --ns <namespace>` (then `aenv cache prune` to reclaim disk for imported skills).
- **Drop the project's pin entirely:** `aenv unpin`.

## What's still in flight

- `aenv install` / `aenv sync` (Phase 6) — pull namespace updates from a git remote so multi-machine sync is automated.

## What to read next

- [Build your own namespace from scratch](./build-your-own.md) — if you don't have a profile to update yet
- [Install a skill from GitHub](./install-a-skill-from-github.md) — the full pin-and-fetch story
- [Snapshot an existing project](./snapshot-an-existing-project.md) — the other direction: capture instead of build
