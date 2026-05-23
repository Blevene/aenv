# Walkthrough: deleting a namespace

**Tested against:** `phase-5-complete` (commit `18eeaec`), `aenv 0.0.1`.
**Goal:** remove a namespace from the registry, understand the warning that fires on every delete, and know which safety nets are NOT yet in place.

`aenv delete <name>` is the only delete-flavored command today. It removes the entire `$AENV_HOME/envs/<name>/` directory tree — manifest, CLAUDE.md, skill directories, everything. The operation is irreversible (no `aenv restore` for deleted namespaces; `restore` is for project-side backups only).

For namespace creation, see `walkthrough-create-namespace.md`. For project disengagement (which is a different operation — removing the `.aenv` pin from a project, not deleting a namespace), see `walkthrough-three-harnesses.md` Step 9 or `aenv unpin --help`.

---

## Prerequisites

```bash
cargo build --release
export AENV_HOME=$(mktemp -d -t aenv-delete-XXXXXX)
export BIN=$PWD/target/release/aenv
export PROJECT=$(mktemp -d -t aenv-delete-proj-XXXXXX)

# Build a small registry with parent/child relationship
$BIN create base --adapter claude-code
$BIN create child --extends base --adapter claude-code

# Populate so child can actually activate later
cat > $AENV_HOME/envs/base/aenv.toml <<'EOF'
name = "base"
[adapters.claude-code]
files = ["CLAUDE.md"]
EOF
echo "# base" > $AENV_HOME/envs/base/CLAUDE.md

cat > $AENV_HOME/envs/child/aenv.toml <<'EOF'
name = "child"
extends = ["base"]
[adapters.claude-code]
files = ["CLAUDE.md"]
EOF
echo "## child" > $AENV_HOME/envs/child/CLAUDE.md

$BIN list
```

```
NAME                   EXTENDS                        ADAPTERS
base                   -                              claude-code
child                  base                           claude-code
```

---

## Happy path: delete an unreferenced namespace

```bash
$BIN create scratch --adapter claude-code
$BIN delete scratch
echo "exit: $?"
```

```
warning: cannot verify namespace 'scratch' is unused; Phase 1 lacks project-tracking. Delete is irreversible.
Deleted namespace 'scratch'
exit: 0
```

Two things to notice:

1. **The warning fires on every delete.** It's printed before the delete runs and is not conditional on anything. Phase 6 will add the project-tracking registry that allows `aenv delete` to refuse on actually-active namespaces (PRD R-4); until then, the warning is your reminder.
2. **Exit code is 0** for a successful delete. The warning is informational, not a refusal.

After delete:

```bash
$BIN list
```

```
NAME                   EXTENDS                        ADAPTERS
base                   -                              claude-code
child                  base                           claude-code
```

The `scratch` directory under `$AENV_HOME/envs/` is gone entirely (manifest + any other files it owned).

---

## Error case: delete a name that doesn't exist (exit code 10)

```bash
$BIN delete does-not-exist
echo "exit: $?"
```

```
warning: cannot verify namespace 'does-not-exist' is unused; Phase 1 lacks project-tracking. Delete is irreversible.
error: namespace not found: does-not-exist
exit: 10
```

The warning still fires (because the project-tracking check is unconditional today), then the actual not-found error returns exit 10.

---

## Edge case: deleting a parent that has children

`child` extends `base`. What happens if we delete `base`?

```bash
$BIN delete base
echo "exit: $?"

$BIN list
```

```
warning: cannot verify namespace 'base' is unused; Phase 1 lacks project-tracking. Delete is irreversible.
Deleted namespace 'base'
exit: 0

NAME                   EXTENDS                        ADAPTERS
child                  base                           claude-code
```

`base` is gone but `child` still extends it. `child` is now an *orphan*. The manifest field `extends = ["base"]` is intact, but resolution will fail when anyone tries to use `child`:

```bash
$BIN use child --project $PROJECT
$BIN activate --project $PROJECT
echo "exit: $?"
```

```
Pinned /tmp/aenv-delete-proj-XXXXXX to namespace 'child'
error: namespace not found: base
exit: 10
```

The pin write succeeded (it's metadata-only); the activation failed at resolution. The fix is either re-create `base` or edit `child`'s manifest to drop the extends reference.

**Note for the future:** Phase 6's project-tracking will add the safety check for "is anything pinned to or extending this?" today the warning is the only protection.

---

## Edge case: deleting a namespace that is actively pinned + materialized

Re-create `base` so the orphan resolves, then activate `child` against the project:

```bash
$BIN create base --adapter claude-code
cat > $AENV_HOME/envs/base/aenv.toml <<'EOF'
name = "base"
[adapters.claude-code]
files = ["CLAUDE.md"]
EOF
echo "# base" > $AENV_HOME/envs/base/CLAUDE.md

$BIN deactivate --project $PROJECT
$BIN use child --project $PROJECT
$BIN activate --project $PROJECT
```

`child` is now active in the project. Try to delete it:

```bash
$BIN delete child
echo "exit: $?"

$BIN list
```

```
warning: cannot verify namespace 'child' is unused; Phase 1 lacks project-tracking. Delete is irreversible.
Deleted namespace 'child'
exit: 0

NAME                   EXTENDS                        ADAPTERS
base                   -                              claude-code
```

Delete proceeded. The project's `.aenv-state/state.json` still references `child`:

```bash
cat $PROJECT/.aenv-state/state.json | python3 -c \
  'import sys,json; d=json.load(sys.stdin); print("state.json active_namespace:", d["active_namespace"])'
```

```
state.json active_namespace: child
```

Querying the project's status fails:

```bash
$BIN status --project $PROJECT
echo "exit: $?"
```

```
error: namespace not found: child
exit: 10
```

This is the most dangerous of the missing safety nets — silently deleting a namespace that's currently materializing files in a real project leaves the project in an unrecoverable status state. Workarounds today:

- **Always** `aenv unpin --project <path>` (or at least `aenv deactivate`) before deleting a namespace that any project is using.
- Phase 6's project-tracking registry will refuse the delete in this case.

`aenv unpin` recovers gracefully even with an orphaned state:

```bash
$BIN unpin --project $PROJECT
```

```
Deactivated namespace in /tmp/aenv-delete-proj-XXXXXX.
Unpinned /tmp/aenv-delete-proj-XXXXXX (was 'child').
```

(The deactivate inside `unpin` cleans up the on-disk managed files and `.aenv-state/`, then the pin file is removed. The orphan state.json was harmless beyond `aenv status` returning exit 10.)

---

## Summary of safety today

| Risk | Status today | Phase 6 plan |
|---|---|---|
| Delete a name that doesn't exist | Returns exit 10 | unchanged |
| Delete a name that's pinned in a project | **Allowed with warning** (orphan state.json possible) | Refuse with exit 13 (ActivationConflict) |
| Delete a name that's extended by another namespace | **Allowed with warning** (orphan child manifest possible) | Refuse OR cascade-warn |
| Delete a name that's been pushed to a remote | n/a (sync deferred) | Phase 6 plans `aenv promote --remove` for shared registries |
| Restore a deleted namespace | **Not possible** without re-creating from external backup | PRD §8 explicitly defers (encrypted/versioned registries out of scope for v1) |

The single sentence: **today, `aenv delete` is `rm -rf $AENV_HOME/envs/<name>/` with a warning. Treat it that way.**

---

## Cleanup

```bash
rm -rf $AENV_HOME $PROJECT
```
