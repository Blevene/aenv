# Walkthrough: creating a namespace

**Tested against:** `main`, `aenv 0.3.0`.
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
files = ["CLAUDE.md"]
```

`--adapter claude-code` also scaffolds an empty `CLAUDE.md` in the namespace dir and lists it in `files`, so a freshly-created namespace materializes a working (if empty) file tree on `aenv activate` with no manual manifest edit. Add more managed paths as you need them:

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
files = ["CLAUDE.md"]
```

`child` will section-merge instructions and deep-merge structured files contributed by `with-adapter` (the parent). Parameters and policies declared on `with-adapter` are inherited unless `child` overrides them.

---

## Form 4: create a global (user-scope) profile (`--global`)

`--global` scaffolds a **user-scope** namespace instead of a project one — the same thing `aenv global new` does, via the unified scope flag (issue #5, Layer 1). Content goes under the namespace's `user/` subtree and materializes into `$HOME` on activation, not into a project.

```bash
$BIN create my-global --global
```

```
Created user-scope namespace 'my-global' at /tmp/aenv-create-XXXXXX/.aenv/envs/my-global
  + user/.claude/CLAUDE.md  (edit this, then run: aenv global use my-global)
```

The generated manifest declares `user_files` (and an empty project `files`):

```toml
name = "my-global"
extends = []

[adapters.claude-code]
files = []
user_files = [".claude/CLAUDE.md"]
```

The on-disk tree is `aenv.toml` + a seeded `user/.claude/CLAUDE.md`. Edit it, then activate with `aenv global use my-global` (or the unified `aenv activate my-global --global`). To make one stored copy serve **both** scopes, rename `user_files` to `shared_files` — see the [global-namespaces walkthrough appendix](./walkthrough-global-namespaces.md#adapter-file-buckets-files--user_files--shared_files). Note `--global` rejects `--extends` (a user-scope scaffold takes no parent on the CLI) and `--project <path>`.

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

(A fresh registry also lists the built-in `cherny` and `karpathy` example namespaces aenv ships; omitted from the example output here for clarity.)

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
    "resolved_hash": "sha256-v1:199230c6d5047c73adbfe2e2cc705a801003159ec0074f66991f90376e0686eb"
  },
  ...
]
```

---

## Observation: hash tracks material content, not name

Look at the `resolved_hash` values across the three namespaces:

- `plain` → `sha256-v1:dc5c79b3...` — no adapter, no files: a truly empty material set.
- `with-adapter` and `child` → `sha256-v1:199230c6...` — **the same hash as each other**, because each resolves to exactly one empty `CLAUDE.md` and nothing else. (`child` extends `with-adapter`, but the parent contributes only that same empty `CLAUDE.md`, so the resolved set is identical.)

That's correct, not a bug: the hash is a function of resolved content + the effective parameter map, not of the namespace name. `plain` differs because its set is empty; `with-adapter` and `child` match because their sets are byte-for-byte identical. Add a managed file, an instruction line, or a parameter to any of them and its hash diverges immediately.

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
