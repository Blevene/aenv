# Walkthrough: three harnesses on one project

**Tested against:** `phase-5-complete` (commit `18eeaec`), `aenv 0.0.1`.
**Goal:** stand up three distinct AI-coding harnesses for the same Rust project, swap between them, observe the bytes that change on disk per activation, capture a content hash per harness, and disengage cleanly.

This walkthrough is the smoke test for the §7.5 *scripted comparison* use case from the functional spec — it reproduces the workflow a downstream evaluation tool would automate, but executed by hand so each surface is observable.

---

## Prerequisites

Build the release binary (the walkthrough uses absolute paths to it, so it doesn't matter what's on `PATH`):

```bash
cargo build --release
```

Pick a scratch `AENV_HOME` so this walkthrough doesn't touch your real `~/.aenv/`:

```bash
export AENV_HOME=$(mktemp -d -t aenv-walk-XXXXXX)
export PROJECT=/path/to/your/rust/project   # use any project you can write to
export BIN=$PWD/target/release/aenv

$BIN --version
# → aenv 0.0.1
```

The rest of the commands assume these three env vars are set.

---

## Step 1 — create three namespaces

The two flags `--extends <parent>` and `--adapter <name>` mean each create writes a usable skeleton manifest in one shot. `--adapter` validates against the installed adapter registry; an unknown name fails with exit 11 before any file is written.

```bash
$BIN create base --adapter claude-code
$BIN create detailed-execution --extends base --adapter claude-code
$BIN create experiments --extends base --adapter claude-code
```

```
Created namespace 'base' at /tmp/aenv-walk-…/envs/base
Created namespace 'detailed-execution' at /tmp/aenv-walk-…/envs/detailed-execution
Created namespace 'experiments' at /tmp/aenv-walk-…/envs/experiments
```

`aenv list` now shows the three-column registry view:

```bash
$BIN list
```

```
NAME                   EXTENDS                        ADAPTERS
base                   -                              claude-code
detailed-execution     base                           claude-code
experiments            base                           claude-code
```

---

## Step 2 — populate manifests + CLAUDE.md content

This step is still manual today: `--adapter` seeds the `[adapters.claude-code]` block with `files = []`, but you fill in the list and the CLAUDE.md body.

`$AENV_HOME/envs/base/aenv.toml`:

```toml
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-sonnet-4.6"
instructions_budget = 5000
```

`$AENV_HOME/envs/base/CLAUDE.md`:

```markdown
## Project Facts

`aenv` is a Rust CLI workspace at `crates/aenv-core/` and `crates/aenv-cli/`.

## Build & Test

- `cargo build --workspace`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
```

`$AENV_HOME/envs/detailed-execution/aenv.toml`:

```toml
name = "detailed-execution"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-opus-4.7"
instructions_budget = 3000

[policies]
instructions_max_chars = { value = 3000, enforce = true }
```

`$AENV_HOME/envs/detailed-execution/CLAUDE.md`:

```markdown
## Disposition

Be careful. Read related code before editing. Small focused commits.
```

`$AENV_HOME/envs/experiments/aenv.toml`:

```toml
name = "experiments"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-sonnet-4.6"
```

`$AENV_HOME/envs/experiments/CLAUDE.md`:

```markdown
## Disposition

Be broad. Try multiple approaches. Sketch first, refine later.
```

---

## Step 3 — pin + activate

Pinning (`aenv use`) and activation (`aenv activate`) are two separate steps by design. Pinning writes metadata (`.aenv` at the project root) and always succeeds; activation can fail and roll back. The §5.2 functional spec story explains the why.

```bash
$BIN use detailed-execution --project $PROJECT
$BIN activate --project $PROJECT
```

```
Pinned /path/to/your/rust/project to namespace 'detailed-execution'
Activated 'detailed-execution' in /path/to/your/rust/project
  + CLAUDE.md (SectionMerge)
```

The `SectionMerge` strategy fired because both `base` and `detailed-execution` declared `CLAUDE.md`. The merged result is a regular file (not a symlink) since two namespaces contribute.

---

## Step 4 — inspect the active state

`aenv status` (text):

```bash
$BIN status --project $PROJECT
```

```
Active namespace: detailed-execution
Resolution:       base → detailed-execution

Managed files:
  ./CLAUDE.md
      merged from base + detailed-execution

Parameters:
  default_model                  = claude-opus-4.7 (from detailed-execution)
  instructions_budget            = 3000 (from detailed-execution)

Active policies:
  instructions_max_chars         = 3000 (from detailed-execution) enforce=true
```

The same data, scriptable form (`--json` is the source of truth for downstream tools):

```bash
$BIN status --project $PROJECT --json
```

```json
{
  "project": "/path/to/your/rust/project",
  "active_namespace": "detailed-execution",
  "resolution_chain": ["base", "detailed-execution"],
  "resolved_hash": "sha256-v1:85ee655ddd8f3f5b5c246fc8336ee0bee46be1b7158e02943b6e115d89367cb0",
  "parameters": {
    "default_model": {
      "value": "claude-opus-4.7",
      "source_namespace": "detailed-execution",
      "inheritance_chain": [
        {"namespace": "base", "value": "claude-sonnet-4.6"},
        {"namespace": "detailed-execution", "value": "claude-opus-4.7"}
      ]
    },
    "instructions_budget": {
      "value": 3000,
      "source_namespace": "detailed-execution",
      "inheritance_chain": [
        {"namespace": "base", "value": 5000},
        {"namespace": "detailed-execution", "value": 3000}
      ]
    }
  },
  "policies": {
    "instructions_max_chars": {
      "value": 3000,
      "enforce": true,
      "source": "detailed-execution"
    }
  },
  "managed_files": [
    {
      "path": "CLAUDE.md",
      "qualified_name": "(merged)::CLAUDE.md",
      "short_name": "CLAUDE.md",
      "provided_by_namespace": null,
      "strategy": "section-merge",
      "contributors": ["base::CLAUDE.md", "detailed-execution::CLAUDE.md"]
    }
  ],
  "backed_up": []
}
```

Each parameter entry carries its `source_namespace` and the full `inheritance_chain` of every namespace in the resolution that declared it — a downstream tool gets provenance without a second `aenv get` call.

`aenv which` for the merged file:

```bash
$BIN which CLAUDE.md --project $PROJECT
```

```
Qualified name:  (merged)::CLAUDE.md
Materialized at: ./CLAUDE.md
Strategy:        section-merge
Contributors:    base::CLAUDE.md
                 detailed-execution::CLAUDE.md
```

For symlink-strategy files (single-namespace contribution), `aenv which` additionally shows the absolute `Source path:` inside the namespace directory. For merged files there's no single source, so the line is omitted and `Contributors:` is what you read instead.

`aenv get` for a single parameter, from anywhere on the filesystem:

```bash
cd /tmp
$BIN get .default_model --project $PROJECT
```

```
claude-opus-4.7
  source: detailed-execution (overrides base which declared claude-sonnet-4.6)
```

`aenv doctor` evaluates every active policy:

```bash
$BIN doctor --project $PROJECT
```

```
Namespace 'detailed-execution' (resolution: base → detailed-execution)

Active policies (after inheritance):
  instructions_max_chars         = 3000 (from detailed-execution) enforce=true

2 pass, 0 warn, 0 fail, 0 skipped.
No issues found.
```

---

## Step 5 — the §7.5 hash loop

The headline scriptability demo: a downstream evaluation tool would loop over the namespaces, activate each, capture `resolved_hash` for the harness used in each run, save it alongside the agent's outputs for later reproducibility checks.

```bash
for ns in base detailed-execution experiments; do
  $BIN deactivate --project $PROJECT >/dev/null
  $BIN use $ns --project $PROJECT >/dev/null
  $BIN activate --project $PROJECT >/dev/null
  H=$($BIN status --project $PROJECT --json | jq -r .resolved_hash)
  CHAIN=$($BIN status --project $PROJECT --json | jq -r '.resolution_chain | join(" → ")')
  echo "$ns ($CHAIN): $H"
done
```

```
base (base): sha256-v1:bdcac8e67b0346fafee318381b1ebdb3c8b16d97cade29065edbcb0105add3b8
detailed-execution (base → detailed-execution): sha256-v1:85ee655ddd8f3f5b5c246fc8336ee0bee46be1b7158e02943b6e115d89367cb0
experiments (base → experiments): sha256-v1:ef47d6162bb1903aa42785d20ef751de5faa8a8916797aec198a1fdd3aa0c4a1
```

Three distinct hashes for three distinct resolved namespaces. Re-running the loop produces the same three hashes — the hash is a function of resolved content + effective parameter map, not of activation timestamps or any other run-local state.

(If you change a manifest or any namespace-side file, the hash for that namespace changes. If you change the on-disk project files without going through `aenv`, the hash doesn't change — `aenv diff` reports that as drift, see Step 7.)

---

## Step 6 — structural diff between two namespaces

`aenv diff <a> <b>` reports the structural difference between two resolved namespaces: parameter changes, added/removed policies, added/removed instructions sections, added/removed skills.

```bash
$BIN diff base detailed-execution
```

```
Parameters:
  default_model: "claude-sonnet-4.6" → "claude-opus-4.7"
  instructions_budget: 5000 → 3000

Policies:
  +instructions_max_chars: 3000

Instructions sections:
  + ## Disposition
```

Same command, `--json` form for tooling consumption is available with `--json`.

---

## Step 7 — enforce-policy violation + rollback

`detailed-execution` declared `instructions_max_chars = 3000` with `enforce = true`. Let's break it: bloat `detailed-execution/CLAUDE.md` past 3000 chars and try to activate.

```bash
python3 -c "print('## Disposition\nBe careful.\n\n' + ('## Padding\n\n' + 'lorem ipsum ' * 200) * 3)" \
  > $AENV_HOME/envs/detailed-execution/CLAUDE.md
wc -c $AENV_HOME/envs/detailed-execution/CLAUDE.md
# → 7265 /tmp/.../envs/detailed-execution/CLAUDE.md

$BIN deactivate --project $PROJECT
$BIN activate detailed-execution --project $PROJECT
echo "exit code: $?"
```

```
error: policy violation: [instructions_max_chars] detailed-execution::CLAUDE.md: CLAUDE.md has 7265 chars (budget 3000). Refactor procedural content into skills, dispositional content into subagents, or use @-imports.
exit code: 17
```

The error message names the policy, the qualified target, the actual-vs-budget chars, and a refactoring hint. Exit code 17 (policy-violation) is part of the public contract per PRD R-82.

Rollback is complete: no `CLAUDE.md` was written, no `state.json` was created.

```bash
ls $PROJECT/CLAUDE.md $PROJECT/.aenv-state/state.json 2>&1
# → ls: cannot access ...: No such file or directory  (both)
```

Restore the small `CLAUDE.md` to proceed:

```bash
cat > $AENV_HOME/envs/detailed-execution/CLAUDE.md <<'EOF'
## Disposition

Be careful. Read related code before editing. Small focused commits.
EOF
```

---

## Step 8 — drift detection

Switch to `experiments` and demonstrate `aenv diff` (no args = drift mode):

```bash
$BIN use experiments --project $PROJECT
$BIN activate --project $PROJECT

$BIN diff --project $PROJECT
```

```
No drift detected. All managed files match their namespace source.
```

Now simulate a user editing the merged `CLAUDE.md` in place (a common mistake — the file looks like a regular file because it is, but the next activation regenerates it from the merge inputs):

```bash
cat >> $PROJECT/CLAUDE.md <<'EOF'

## Local edits
These will be lost on next activate.
EOF

$BIN diff --project $PROJECT
```

```
Drift in project /path/to/your/rust/project:
  CLAUDE.md (merge-regenerated)
    396 bytes on disk vs 343 bytes expected (17 vs 14 lines)
```

Same again with `--json`:

```bash
$BIN diff --project $PROJECT --json
```

```json
{
  "project": "/path/to/your/rust/project",
  "active_namespace": "experiments",
  "drifted": [
    {
      "path": "CLAUDE.md",
      "qualified_name": "(merged)::CLAUDE.md",
      "kind": "merge-regenerated",
      "summary": "396 bytes on disk vs 343 bytes expected (17 vs 14 lines)"
    }
  ]
}
```

Drift kinds: `symlink-replaced` (managed-as-symlink file is now a regular file with different bytes), `merge-regenerated` (managed-as-merged file's bytes don't match a fresh merge), `content-divergent` (catch-all for `Copy`/`Identical` and edge cases).

`aenv diff` always exits 0 today regardless of drift — consumers parse the JSON `drifted` array to decide. (A future `--exit-code` flag mirroring `git diff --exit-code` is on the backlog.)

---

## Step 9 — disengage with `aenv unpin`

To fully disengage `aenv` from the project — deactivate the namespace, remove `.aenv-state/`, remove the `.aenv` pin — one command does it:

```bash
$BIN unpin --project $PROJECT
```

```
Deactivated namespace in /path/to/your/rust/project.
Unpinned /path/to/your/rust/project (was 'experiments').
```

Confirm:

```bash
ls -la $PROJECT/.aenv $PROJECT/.aenv-state $PROJECT/CLAUDE.md 2>&1
# → all three: cannot access: No such file or directory
```

If you `unpin` a project that wasn't pinned, the command is idempotent (exits 0 with `No namespace pinned in <path>.`). If you `unpin` while a namespace is pinned but not active, it just removes the pin file.

---

## Step 10 — cleanup

Remove the scratch registry:

```bash
rm -rf $AENV_HOME
```

The project is back to its pre-walkthrough state.

---

## What you've just confirmed

This walkthrough exercises every PRD §5 surface except the Phase 6 work (shell hook, remote sync) and the Phase 4 skill-import work (which has its own dedicated tests). Specifically:

- §5.1 lifecycle: `create` with `--extends` and `--adapter`, `list` text output.
- §5.2 pinning: `use` writes `.aenv`.
- §5.6 file materialization: section-merge produces a regular file with both contributors; symlink for single-contributor cases.
- §5.7 activation: explicit `activate` step, two-step model.
- §5.9 status and introspection: `status` text + JSON, `which`, `get` with inheritance chain.
- §5.12 safety: enforce-policy gate fires before any file write, exit 17, complete rollback.
- §5.14 parameters: typed parameters with inheritance + override.
- §5.15 policies: `doctor`, `enforce = true`, inheritance.
- §5.16 scriptability: `--json` on every read command; `--project` plumbed end-to-end.
- §5.17 hash: `sha256-v1:<hex>` is stable across re-activation and distinct between namespaces.
- §7.5 scripted comparison: the three-namespace activation loop with hash capture is the inner loop of a downstream eval tool.

The whole sequence runs in under 30 seconds on a developer laptop. Every command is scriptable via the `--json` output. If you swap the manual CLAUDE.md edits for a build step that writes them, the entire walkthrough becomes a one-shot CI smoke test.

---

## Honest remaining friction

This walkthrough is the post-polish experience. Things still missing today that you'd notice if you scaled this to a real production workflow:

- **The `aenv` pin file and the project's `.git` repo coexist quietly here**, but in a project that ignores `.aenv` by convention, you may want to add it to `.gitignore` (or commit it, if your team shares the pin). The functional spec doesn't take a side.
- **No `aenv install` yet** — the Phase 6 remote-sync surface would let a teammate clone the repo and `aenv install` to fetch the namespaces. Today the registry layout under `$AENV_HOME/envs/` has to be set up by hand or by some out-of-band sync.
- **No shell hook yet** — Phase 6 will add `aenv init-shell bash|zsh|fish` so that `cd` into a pinned directory auto-activates. Today the `aenv use && aenv activate` pair is manual.
- **The `aenv list` ADAPTERS column shows comma-separated adapter names but doesn't show declared parameters or policies.** `--json` does — for tabular text discovery beyond that, `aenv get` + `aenv doctor` per namespace.

These are scoped to Phase 6 / 7. The current `phase-5-complete` state is what you'd dogfood today for a single-machine, single-user harness-comparison workflow.
