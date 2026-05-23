# Walkthrough: creating a namespace

**Tested against:** `phase-5-complete` (commit `18eeaec`), `aenv 0.0.1`.
**Goal:** create a new namespace in the registry, from the minimum viable form (one command) up to the full form (one command with extends + adapter pre-seeded).

A *namespace* in `aenv` is a directory under `$AENV_HOME/envs/<name>/` containing an `aenv.toml` manifest and any associated harness files (CLAUDE.md, skill SKILL.md files, etc.). This walkthrough covers `aenv create` and its two flags.

For the broader story (creating multiple namespaces, activating them against a project, comparing them), see `walkthrough-three-harnesses.md`.

---

## Prerequisites

Build the release binary and pick a scratch registry:

```bash
cargo build --release
export AENV_HOME=$(mktemp -d -t aenv-create-XXXXXX)
export BIN=$PWD/target/release/aenv
```

---

## Form 1: minimum viable create

The simplest invocation. Produces a manifest with no adapters and no extends — useful when you want to fully shape the namespace by hand.

```bash
$BIN create plain
```

```
Created namespace 'plain' at /tmp/aenv-create-XXXXXX/envs/plain
```

The generated manifest:

```bash
cat $AENV_HOME/envs/plain/aenv.toml
```

```toml
name = "plain"
extends = []

[adapters]
```

Empty `[adapters]` block. You'd hand-edit it to add `[adapters.claude-code] files = [...]` before this namespace did anything useful. The cleaner option is Form 2.

---

## Form 2: create with adapter pre-seeded

`--adapter <name>` seeds an empty `[adapters.<name>]` block. The flag is repeatable. Each name is validated against the installed adapter registry; an unknown name fails fast (exit code 11, AdapterMissing) before any file is written.

```bash
$BIN create with-adapter --adapter claude-code
```

```
Created namespace 'with-adapter' at /tmp/aenv-create-XXXXXX/envs/with-adapter
```

The generated manifest:

```toml
name = "with-adapter"
extends = []

[adapters.claude-code]
files = []
```

The `files = []` is your next edit — list which paths in the project this namespace's `claude-code` adapter should manage:

```toml
[adapters.claude-code]
files = ["CLAUDE.md", ".claude/skills/**/*"]
```

---

## Form 3: create with extends + adapter (composition-ready)

`--extends <parent>` declares a parent namespace to inherit from. The flag is repeatable for diamond inheritance.

```bash
$BIN create child --extends with-adapter --adapter claude-code
```

```
Created namespace 'child' at /tmp/aenv-create-XXXXXX/envs/child
```

The generated manifest:

```toml
name = "child"
extends = ["with-adapter"]

[adapters.claude-code]
files = []
```

`child` will section-merge instructions and deep-merge structured files contributed by `with-adapter` (the parent). Parameters and policies declared on `with-adapter` are inherited unless `child` overrides them.

---

## Error cases

### Unknown adapter (exit code 11)

```bash
$BIN create bad --adapter not-an-adapter
echo "exit: $?"
```

```
error: adapter not installed: not-an-adapter
exit: 11
```

No directory is created when the adapter check fails. Verify:

```bash
ls $AENV_HOME/envs/bad 2>&1
```

```
ls: cannot access '/tmp/.../envs/bad': No such file or directory
```

### Duplicate name (exit code 12)

```bash
$BIN create plain
echo "exit: $?"
```

```
error: manifest invalid: namespace 'plain' already exists
exit: 12
```

`aenv create` refuses to overwrite. To replace a namespace, `aenv delete <name>` first (see `walkthrough-delete-namespace.md` for the safety story).

---

## Verify with `aenv list`

After the three successful creates above:

```bash
$BIN list
```

```
NAME                   EXTENDS                        ADAPTERS
child                  with-adapter                   claude-code
plain                  -                              -
with-adapter           -                              claude-code
```

Or the scriptable form:

```bash
$BIN list --json
```

```json
[
  {
    "name": "child",
    "extends": ["with-adapter"],
    "adapters": ["claude-code"],
    "parameters_declared": [],
    "policies_declared": [],
    "resolved_hash": "sha256-v1:dc5c79b33cb25c8b033eea26a3daa383720babd9c506ca24b2bf4428888ff743"
  },
  ...
]
```

---

## Observation: all three new namespaces hash to the same value

Look closely at `list --json`: `plain`, `with-adapter`, and `child` all produce the same `resolved_hash` (the `dc5c79b3...` value above).

That's correct, not a bug. Each namespace's material set is empty (no `files = [...]` populated, no parameters, no skills) so the hash input is the same: an empty file set plus an empty `.aenv/parameters.json`. As soon as you add a managed file or a parameter to any of them, the hashes diverge.

This is the §5.17 invariant working: hash is content-determined, not name-determined.

---

## Next steps

After `aenv create`, the typical next moves are:

- **Populate the manifest:** add `files = [...]` to adapter blocks, declare parameters, declare policies. See `walkthrough-modify-namespace.md`.
- **Author or import skills:** `aenv skill new <name> --ns <ns>` for inline, `aenv skill import <source> --ns <ns>` for external. Covered in `walkthrough-modify-namespace.md`.
- **Activate against a project:** `aenv use <name>` + `aenv activate`. Covered in `walkthrough-three-harnesses.md`.

---

## Cleanup

```bash
rm -rf $AENV_HOME
```
