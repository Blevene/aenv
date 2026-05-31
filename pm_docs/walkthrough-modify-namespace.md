# Walkthrough: modifying a namespace

**Tested against:** `main`, `aenv 0.3.0`.
**Goal:** mutate an existing namespace — add managed files, parameters, policies, authored skills, and imported skills — using the right tool for each kind of change.

The vocabulary deliberately tracks where the source of truth lives:

- **Adapter files** (e.g., `files = ["CLAUDE.md"]`): manifest edit. No CLI command today; you edit `aenv.toml` directly.
- **Parameters** (typed values like `default_model = "claude-opus-4.7"`): `aenv set <ns>.<param> <value>`.
- **Policies** (validation rules like `instructions_max_chars = ...`): manifest edit. No CLI command today.
- **Authored skills**: `aenv skill new <name> --ns <ns>`.
- **Imported skills**: `aenv skill import <source> --ns <ns> [--pin <ref>]`.

For namespace creation, see `walkthrough-create-namespace.md`. For deletion, see `walkthrough-delete-namespace.md`.

---

## Prerequisites

```bash
cargo build --release
export AENV_HOME=$(mktemp -d -t aenv-modify-XXXXXX)
export BIN=$PWD/target/release/aenv

$BIN create base --adapter claude-code
```

You now have an empty namespace at `$AENV_HOME/envs/base/` with a skeletal manifest. The next sections add to it.

---

## 1. Add managed files (manifest edit)

`--adapter claude-code` seeded an empty `[adapters.claude-code] files = []` block. Fill it in by editing `aenv.toml`:

```bash
cat > $AENV_HOME/envs/base/aenv.toml <<'EOF'
name = "base"
extends = []

[adapters.claude-code]
files = ["CLAUDE.md"]
EOF

cat > $AENV_HOME/envs/base/CLAUDE.md <<'EOF'
## Project Facts

Initial fact set.
EOF
```

The two-file pattern is canonical:
- The manifest declares *which* paths the adapter manages.
- The actual files live under the namespace directory at those same relative paths.

Multiple files are fine. Globs are fine (e.g., `.claude/skills/**/*` is the convention for adapter-managed skill directories).

---

## 2. Add parameters via `aenv set`

Parameters are typed values that resolve through the `extends` chain (last-wins per key). Four types are supported: string, integer, boolean, list-of-string.

```bash
$BIN set base.default_model claude-sonnet-4.6
```

```
Set base.default_model
```

```bash
$BIN set base.instructions_budget 5000
```

```
Set base.instructions_budget
```

The manifest now carries the `[parameters]` table:

```bash
cat $AENV_HOME/envs/base/aenv.toml
```

```toml
name = "base"
extends = []

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-sonnet-4.6"
instructions_budget = 5000
```

Read parameters back with `aenv get`:

```bash
$BIN get base.default_model
```

```
claude-sonnet-4.6
  source: base (declared, not inherited)
```

`aenv set` infers the type from the value (`true`/`false` → boolean, digits → integer, `[a, b]` → list-of-string, else string). For an integer parameter that an adapter has declared as a particular type, the resolver will refuse a type-incompatible value at activation time with exit 12 (ManifestInvalid).

---

## 3. Add policies (manifest edit)

Policies are validation rules `aenv doctor` evaluates. There's no `aenv set`-equivalent for them today — you edit the manifest's `[policies]` block. The four built-in policy keys are:

- `instructions_max_chars` — integer
- `skill_requires_description` — boolean
- `mcp_requires_command_or_url` — boolean
- `forbid_paths` — list-of-string

Each takes either a shorthand value (advisory) or the long form `{ value = ..., enforce = true }` (enforced — blocks activation on violation, exit 17).

```bash
cat > $AENV_HOME/envs/base/aenv.toml <<'EOF'
name = "base"
extends = []

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-sonnet-4.6"
instructions_budget = 5000

[policies]
instructions_max_chars = { value = 5000, enforce = true }
EOF
```

Verify with `aenv doctor`:

```bash
$BIN doctor base
```

```
Namespace 'base' (resolution: base)

Active policies (after inheritance):
  instructions_max_chars         = 5000 (from base) enforce=true

1 pass, 0 warn, 0 fail, 0 skipped.
No issues found.
```

Note: `aenv doctor <namespace>` is the explicit-target form (no `--project` required). `aenv doctor --project <path>` evaluates the project's pinned namespace.

---

## 4. Add an authored skill

Authored skills live in the namespace directory under the adapter's `skills_dir` (for `claude-code`, that's `.claude/skills/<skill-name>/`).

```bash
$BIN skill new write-tests --ns base
```

```
Created authored skill 'write-tests' in namespace 'base':
  - /tmp/aenv-modify-XXXXXX/envs/base/.claude/skills/write-tests/SKILL.md
  - registered in /tmp/aenv-modify-XXXXXX/envs/base/aenv.toml
```

The command did three things:

1. Created the directory `$AENV_HOME/envs/base/.claude/skills/write-tests/`.
2. Wrote a stub `SKILL.md` with adapter-appropriate frontmatter.
3. Appended a `[[skills]]` entry to the namespace's `aenv.toml`.

The stub:

```bash
cat $AENV_HOME/envs/base/.claude/skills/write-tests/SKILL.md
```

```markdown
---
name: write-tests
description: TODO: describe this skill
---

# write-tests

Describe when the agent should invoke this skill.
```

Edit the `description` and the body. The `description` is what makes the skill discoverable to the agent at runtime — leave it as `TODO:` and `skill_requires_description` (if enabled as an enforce policy) will block activation.

The manifest now records the skill:

```toml
[[skills]]
name = "write-tests"
mode = "authored"
adapter = "claude-code"
required = false
```

---

## 5. Add an imported skill from a local path

Imported skills live outside the namespace directory — they're resolved at activation time. Sources can be local paths, `git+URL[#ref]`, or `registry:<name>` (the registry source type is stubbed in Phase 4).

Set up a local skill source to import:

```bash
SKILL_SRC=$(mktemp -d -t skill-src-XXXXXX)
mkdir -p $SKILL_SRC/match-conventions
cat > $SKILL_SRC/match-conventions/SKILL.md <<'EOF'
---
name: match-conventions
description: Match the surrounding code conventions.
---
Read 2-3 nearby files before editing. Mirror their style.
EOF
```

Import it:

```bash
$BIN skill import $SKILL_SRC/match-conventions --ns base
```

```
Imported skill 'match-conventions' into namespace 'base':
  - source: /tmp/skill-src-XXXXXX/match-conventions
  - no pin (resolves on each activation)
  - registered in /tmp/aenv-modify-XXXXXX/envs/base/aenv.toml
```

The manifest appends a second `[[skills]]` entry with `mode = "imported"`:

```toml
[[skills]]
name = "match-conventions"
mode = "imported"
adapter = "claude-code"
source = "/tmp/skill-src-XXXXXX/match-conventions"
required = false
```

For a git-sourced import with version pinning:

```bash
$BIN skill import git+https://github.com/example/aenv-skills.git#match-conventions \
  --ns base --pin v1.2.0
```

The `--pin <ref>` option resolves the ref at import time and records the resolved SHA in the manifest's `ref` field so the activation is reproducible across machines. Without `--pin`, the skill resolves to head on each activation.

For `required = true` semantics (unreachable import → fail activation with exit 13 instead of warn + skip), add `required = true` to the manifest entry by hand — `aenv skill import` doesn't have a flag for this today.

---

## 6. List + introspect

Show the skill roster:

```bash
$BIN skill list --ns base
```

```
NAMESPACE             SKILL                           MODE        SOURCE                                                        PIN
base                  write-tests                     authored    -                                                             -
base                  match-conventions               imported    /tmp/skill-src-XXXXXX/match-conventions                       (head)
```

The scriptable form:

```bash
$BIN skill list --ns base --json
```

```json
[
  {
    "namespace": "base",
    "qualified_name": "base::write-tests",
    "short_name": "write-tests",
    "adapter": "claude-code",
    "mode": "authored",
    "required": false
  },
  {
    "namespace": "base",
    "qualified_name": "base::match-conventions",
    "short_name": "match-conventions",
    "adapter": "claude-code",
    "mode": "imported",
    "source": "/tmp/skill-src-XXXXXX/match-conventions",
    "pin": "(head)",
    "required": false
  }
]
```

---

## What's NOT yet modifiable via CLI

| Surface | Today | Future |
|---|---|---|
| Adapter `files = [...]` list | Manifest edit | Possibly `aenv adapter add-file <ns> <path>` someday |
| Policies | Manifest edit | Possibly `aenv set <ns>.policy.<key> ...` symmetric with parameters |
| Rename a namespace | Not supported | Out of scope (PRD §8) |
| Move a namespace's parent (`extends`) | Manifest edit | Same — manifest is the source of truth |
| `aenv skill new --required` | Not a flag | Add via manifest edit |
| `aenv skill refresh` (re-resolve unpinned imports) | Not implemented | Listed as Phase 4 deferred |

For everything in the "Manifest edit" column, the canonical pattern is: edit `aenv.toml`, then verify with `aenv doctor <ns>` or `aenv list`.

---

## Cleanup

```bash
rm -rf $AENV_HOME $SKILL_SRC
```
