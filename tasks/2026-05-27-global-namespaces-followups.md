# Global Namespaces Follow-ups — Daily-Driver Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> Project precedent: `cargo` is not on PATH. Every cargo invocation must be prefixed with `PATH="$HOME/.cargo/bin:$PATH"`. Style: `rustfmt max_width = 100`, `clippy -D warnings`. Commits land directly on `main`. Don't push, don't run destructive git ops.

**Goal:** Turn Issue #4's global-namespaces mechanism into a feature that actually delivers user value — specifically, the ability to use a complex, fully-instrumented harness (`juanandresgs/claude-ctrl`) as a daily-driver alternative to a snapshot of the user's existing `~/.claude/` setup, with reliable swap, install-time dependency setup, and a guaranteed recovery path when things go wrong.

**Why the current implementation isn't sufficient:** As surfaced in the 2026-05-27 live demo, `aenv global activate` only moves file positions — it doesn't run installers, doesn't verify that hook-referenced binaries exist, and offers no in-session recovery when the active environment's hooks lock the user out of their shell. claude-ctrl's policy engine needs a Python runtime installed by its `install.sh`; without that, activation succeeds mechanically but produces a non-functional environment. The mechanism works; the workflow doesn't.

**Bar for "done":** A new user can run `aenv global snapshot default` → `aenv global import https://github.com/juanandresgs/claude-ctrl claude-cntrl` → `aenv global activate claude-cntrl` → use Claude Code with claude-ctrl active for days → `aenv global activate default` → restored. No manual `cp`, no manual `pip install`, no second terminal, no surprises.

**Architecture deltas vs Issue #4:**
- Namespace manifest gains `[lifecycle]` section (`on_activate`, `on_deactivate`).
- Activation state gains `lifecycle_approved: bool` + `lifecycle_ran: bool` fields. `SCHEMA_VERSION` 5 → 6.
- New CLI verbs: `aenv global snapshot`, `aenv global import`, `aenv global deactivate --force`.
- Companion script: `aenv-rescue` (POSIX sh) — direct fs ops, no aenv binary needed; reads `~/.aenv/global-state.json` and undoes activation without going through Claude Code.
- New manifest field: `[adapters.<name>] materialize = "copy" | "symlink"` per-adapter override of the default symlink strategy.
- Builtin claude-code adapter stays minimal; claude-cntrl namespace declares its own runtime paths (allowed because no containment check; F10 makes this explicit in docs).

**Tech stack:** Same as Issue #4 — Rust, `clap` v4 derive, `serde` + `toml`, `serde_json`, `thiserror`, `tempfile` + `insta` + `proptest` for tests. New addition: a small POSIX-sh script for `aenv-rescue`.

**Public-contract changes (all additive, no breaks):**
- `SCHEMA_VERSION` 5 → 6 (`lifecycle_approved`, `lifecycle_ran` fields on `ActivationState`).
- No new exit codes; reuse `GlobalConflict` (19) for "lifecycle hook failed" and "activation rolled back."
- New JSON fields on `aenv global which --json` (`content_hash`).
- New manifest sections: `[lifecycle]`, `[adapters.<name>] materialize = "copy"`.

**Sequencing principle:** Same as original Issue #4 plan — each task ships testable software; later tasks build on earlier. Tasks within a milestone can land in one commit if they're tightly coupled; across milestones, prefer separate commits for review-ability.

---

## Milestone groups

- **I. Foundation** (Tasks 1–2): Document the `user_files` extensibility (F10); add `ManagedFile.was_present_before_activation` flag downstream tasks rely on. ~0.5 day.
- **J. Snapshot + Import** (Tasks 3–6): `aenv global snapshot` + `aenv global import` (local path + git URL) with `aenv-namespace.toml` convention file fallback to heuristic. (F1 + F9) ~2 days.
- **K. Lifecycle hooks** (Tasks 7–12, **plus 12.5**): `[lifecycle]` schema, activator + deactivator integration, namespace-scoped + SHA-pinned approval prompt, rollback semantics, mini fixture + real claude-ctrl `#[ignore]`-gated test. (F2) ~2.5 days.
- **L. Recovery** (Tasks 13–15): `aenv global deactivate --force` + `aenv-rescue` **Rust binary** + recovery docs. (F3) ~1 day.
- **M. Pre-flight safety** (Tasks 16–18): Settings.json command-path scanning (hooks, MCP, statusLine, advisors), doctor integration, activation pre-flight prompt. (F5 + F8) ~1.5 days.
- **N. Content hash in `which`** (Task 19): Adds resolved-bytes hash so the harness-eval consumer can detect drift per file. (F6) ~0.5 day.
- **O. Copy strategy** (Tasks 20–22): Per-namespace `materialize = "copy"` opt-in, replacing the current Phase-7-deferred hard-error. (F7) **~2 days** (bumped from 1.5 after auditing the `MaterializeStrategy::Copy` placeholder).
- **P. Docs + release** (Tasks 23–26): Honest rewrite of README + walkthrough using claude-cntrl end-to-end; CHANGELOG; version bump; tag; close issue. ~1 day.

**Estimated total: ~11 working days** (revised from 10 after the Task 20 estimate bump and the Task 12.5 addition).

**Plus Stretch tasks (S1–S5)** filed as separate follow-up issues post-v0.1.0. Estimated 2–3 additional days each, none blocking MVP.

---

# Milestone I — Foundation

## Task 1: Document namespace `user_files` extensibility (F10)

**Files:**
- Modify: `pm_docs/aenv-functional-spec.md` (or wherever the manifest schema is documented — verify before editing)
- Modify: `README.md` (Global namespaces section)
- Modify: `crates/aenv-core/src/manifest.rs` (doc comment on `AdapterEntry.user_files`)

**Why:** Today the resolver allows a namespace's `[adapters.<name>] user_files = [...]` to declare paths the adapter itself didn't list. This is load-bearing for claude-cntrl, which declares `.claude/runtime/`, `.claude/bin/`, `.claude/sidecars/` — none of which appear in the builtin claude-code adapter's `user_files`. Document the behavior explicitly so authors don't assume containment.

- [ ] **Step 1: Audit confirms no containment check**

Run: `grep -rn "user_files\|adapter\.user_files" crates/aenv-core/src/resolve.rs`

Expected: only the gathering loop appears. `validate_candidate_paths` does not check namespace paths against adapter prefixes. (Already confirmed pre-plan.)

- [ ] **Step 2: Update `AdapterEntry.user_files` doc comment**

In `crates/aenv-core/src/manifest.rs`, expand the field's rustdoc from one line to:

```rust
    /// User-scope analog of `files`. Paths are relative to the namespace's
    /// `user/` source subdir, and to `$HOME` at activation time.
    ///
    /// Namespaces MAY declare paths that the adapter's own `user_files`
    /// doesn't list — this lets per-namespace harnesses extend the surface
    /// (e.g. `claude-cntrl` adds `.claude/runtime/` and `.claude/bin/` even
    /// though the builtin claude-code adapter doesn't). No containment check
    /// is enforced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_files: Vec<String>,
```

- [ ] **Step 3: README + walkthrough explainer**

In the README "Global namespaces" section, add a short paragraph titled "Extending the adapter surface" after the verbs table. Two sentences max:

> A namespace's `user_files` is not capped by what the adapter declares. claude-ctrl, for example, declares `.claude/runtime/` in its own manifest even though the builtin claude-code adapter doesn't — aenv materializes any user-scoped path the namespace asks for, as long as it's relative and doesn't escape.

- [ ] **Step 4: Lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4 followup: document user_files extensibility (F10)"
```

No tests — pure documentation.

---

## Task 2: Add `ManagedFile.was_present_before_activation` flag

**Files:**
- Modify: `crates/aenv-core/src/state.rs`
- Modify: `crates/aenv-core/src/activate/mod.rs`, `crates/aenv-core/src/activate/phase1.rs`
- Modify: `crates/aenv-core/src/deactivate.rs`
- Modify: `crates/aenv-core/tests/deactivate.rs` + `tests/activate_global_unit.rs`

**Why:** Today, a path that's `Absent` at activation time gets a symlink created and no `BackedUpFile` entry. On deactivate, the symlink is removed and the path returns to absent. Good. But downstream tasks (M pre-flight, O copy-mode drift detection) need to ask "was this path present before aenv touched it?" — and the `BackedUpFile` table is the wrong place to record that, since `Absent` candidates don't push a backup row.

**Correction from the original critique:** Put the flag on `ManagedFile` (every materialized path gets one), not `BackedUpFile` (only displaced paths get one).

- [ ] **Step 1: Schema + state**

Bump `SCHEMA_VERSION` 5 → 6 in `crates/aenv-core/src/state.rs`.

Add to `ManagedFile`:
```rust
    /// Whether the target path existed (as a file or directory, not as our
    /// own symlink) at the moment of activation. True for the historical
    /// `Displaced`/`ByteIdenticalRegular`/`AlreadyOurSymlink` cases; false
    /// for `Absent`. Defaults to `true` on read for schema-1..5 state files
    /// — preserves historical semantics, since every entry recorded under
    /// those schemas came from a displaced or identical pre-existing file.
    #[serde(default = "default_true")]
    pub was_present_before_activation: bool,
```

Add helper near the existing `Raw` deserializer:
```rust
fn default_true() -> bool { true }
```

- [ ] **Step 2: Set flag at activation**

In `phase1::materialize_symlink`, each `managed.push(ManagedFile { … })` site sets the flag based on the `ProjectPathState` branch:
- `Absent` → `false`.
- `AlreadyOurSymlink`, `ByteIdenticalRegular`, `Displaced` → `true`.

(`BackedUpFile` stays exactly as it is. No flag on it.)

- [ ] **Step 3: Deactivate behavior unchanged**

`deactivate_namespace_in_scope` doesn't need to read the flag — its current behavior of "remove materialized file; restore from backup if a backup row exists" already does the right thing. The flag is read by downstream tasks (Task 17, 22).

- [ ] **Step 4: Tests**

```rust
#[test]
fn managed_file_records_was_present_false_for_absent_target() {
    // Fake home with NO ~/.claude/CLAUDE.md. Activate.
    // Assert state.managed_files[0].was_present_before_activation == false.
    // Deactivate. Assert ~/.claude/CLAUDE.md does not exist.
}

#[test]
fn managed_file_records_was_present_true_for_displaced_target() {
    // Fake home with a pre-existing ~/.claude/CLAUDE.md. Activate (Displaced).
    // Assert state.managed_files[0].was_present_before_activation == true.
}
```

- [ ] **Step 5: Lint + commit**

```
Issue #4 followup: SCHEMA_VERSION 5→6, ManagedFile.was_present_before_activation
```

---

# Milestone J — Snapshot + Import

## Task 3: `aenv global snapshot <name>` core (F1)

**Files:**
- Create: `crates/aenv-core/src/global_snapshot.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Modify: `crates/aenv-cli/src/main.rs` (new `GlobalAction::Snapshot`)
- Create: `crates/aenv-cli/src/cmd/global/snapshot.rs`
- Modify: `crates/aenv-cli/src/cmd/global/mod.rs`
- Create: `crates/aenv-cli/tests/global_snapshot_e2e.rs`

**Why:** First step of any swap is "save my current state." Today it's a manual `mkdir`/`cp`/`vim aenv.toml` dance. Make it one command. Mirror of project-side `aenv snapshot`.

- [ ] **Step 1: Core function**

In `crates/aenv-core/src/global_snapshot.rs`:

```rust
//! User-scope snapshot — captures every adapter-managed path that currently
//! exists under `$HOME` into a new namespace.

pub fn snapshot_global<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    fake_home: &Path,
    name: &str,
    extra_includes: &[String],
) -> Result<()> {
    // 1. Validate namespace name (no collision).
    // 2. Resolve which paths to snapshot: union of every adapter's
    //    user_files plus user_skills_dir, plus `extra_includes`.
    //    Tilde-expand `~/` prefixes against fake_home.
    // 3. For each path that exists under fake_home, copy (recursively
    //    for directories) into envs/<name>/user/<path>.
    // 4. Build a manifest declaring the paths that were copied (skip
    //    missing ones), with one adapter block per adapter contributing.
    // 5. Write the manifest.
}
```

Key design:
- Skip paths that don't exist under `$HOME` — they aren't in the user's environment, so we don't snapshot nothing.
- Copy semantics: use `std::fs::copy` for files, recursive walk for directories. Preserve permissions where possible (best-effort).
- For directories, snapshot is shallow on directory identity but deep on contents. The on-disk result mirrors the original tree.
- Don't snapshot the `~/.aenv/` directory itself (registry-relative paths only).

- [ ] **Step 2: CLI wiring**

In `main.rs`:
```rust
    Snapshot {
        name: String,
        /// Extra paths (relative to $HOME) to include beyond adapter defaults.
        /// Repeatable: --include .claude/runtime --include .claude/bin
        #[arg(long)]
        include: Vec<String>,
    },
```

Dispatcher arm passes `include` as `&[String]` to `cmd::global::snapshot::run`.

- [ ] **Step 3: `cmd::global::snapshot::run`**

Thin wrapper that loads adapters, resolves `$HOME`, calls `snapshot_global`, prints a summary:
```
Snapshotted current ~/.claude/ surface into namespace 'default-blevene' (5 files, 2 directories).
```

- [ ] **Step 4: Tests**

```rust
#[test]
fn global_snapshot_captures_existing_user_files() {
    // Fake home with ~/.claude/CLAUDE.md, ~/.claude/settings.json, ~/.claude/agents/foo.md.
    // Run snapshot.
    // Assert envs/snap-name/user/.claude/ exists with the same files.
    // Assert manifest declares the right user_files.
}

#[test]
fn global_snapshot_skips_missing_paths() {
    // Fake home with ONLY ~/.claude/CLAUDE.md.
    // Run snapshot.
    // Assert manifest only declares .claude/CLAUDE.md, not the other adapter defaults.
}

#[test]
fn global_snapshot_includes_extra_paths() {
    // Fake home with ~/.claude/runtime/cli.py.
    // Run with --include .claude/runtime/.
    // Assert runtime/ is in the snapshot.
}

#[test]
fn global_snapshot_refuses_existing_namespace() {
    // Snapshot once, snapshot again with same name. Second call errors cleanly.
}
```

- [ ] **Step 5: Commit**

```
Issue #4 followup: aenv global snapshot (F1) — capture current ~/.claude/ into a new namespace
```

---

## Task 4: `aenv global snapshot` end-to-end on a real fake $HOME

**Files:**
- Modify: `crates/aenv-cli/tests/global_snapshot_e2e.rs` (extend)

**Why:** Unit tests assert correctness; e2e proves the binary actually works against a fake `$HOME`. Mirror the existing pattern in `tests/global_use_e2e.rs`.

- [ ] **Step 1: Add e2e test**

Real binary, real tempdir-as-HOME, real adapter, snapshot + activate cycle that proves the snapshot is a usable namespace.

- [ ] **Step 2: Commit**

---

## Task 5: `aenv global import <source> <name>` for local paths (F9 part 1)

**Files:**
- Modify: `crates/aenv-core/src/global_snapshot.rs` (add `import_global` function or similar; same module since they share the "produce a namespace from filesystem content" logic)
- Modify: `crates/aenv-cli/src/main.rs`
- Create: `crates/aenv-cli/src/cmd/global/import.rs`
- Create: `crates/aenv-cli/tests/global_import_e2e.rs`
- Create: `pm_docs/aenv-namespace-toml-spec.md` — convention file format spec.

**Why:** Without import, users wanting to use claude-ctrl have to manually `cp -r /path/to/claude-ctrl-src ~/.aenv/envs/claude-cntrl/user/.claude/` and write the manifest. Make it one command for local paths first; git URLs in Task 6.

### Convention file: `aenv-namespace.toml`

A repo intended as an aenv namespace MAY ship an `aenv-namespace.toml` at its root that tells `aenv global import` exactly what to do. When present, this is authoritative; the heuristic only runs as fallback when it's absent.

Shape (proposed; refine during implementation):

```toml
# At repo root: /path/to/repo/aenv-namespace.toml

# Adapters this namespace touches. Each named adapter must be installed
# in $AENV_HOME/adapters/.
adapters = ["claude-code", "codex"]

# Lifecycle scripts, namespace-relative (paths are kept literally in the
# generated aenv.toml's [lifecycle] section).
[lifecycle]
on_activate = "install.sh"
on_deactivate = "uninstall.sh"

# Layout: source path (in this repo) → target path (under $HOME at activation
# time). Trailing `/` means directory; the importer copies the tree under the
# source into the namespace's user/ subdir at the corresponding target path.
[layout]
"CLAUDE.md"      = ".claude/CLAUDE.md"
"AGENTS.md"      = ".codex/AGENTS.md"
"settings.json"  = ".claude/settings.json"
"agents/"        = ".claude/agents/"
"commands/"      = ".claude/commands/"
"hooks/"         = ".claude/hooks/"
"skills/"        = ".claude/skills/"
"runtime/"       = ".claude/runtime/"
"bin/"           = ".claude/bin/"
"sidecars/"      = ".claude/sidecars/"
".codex/"        = ".codex/"

# Optional: paths inside the source tree to ignore (docs, dev-only files).
ignore = ["docs/", "tests/", "evals/", "MASTER_PLAN.md", "README.md",
          ".env.example", "install*.sh", "package.json", "pyproject.toml"]
```

Behavior:
1. The importer reads this file at the source root.
2. For each `[layout]` entry whose source path exists in the repo, copy the tree to `envs/<name>/user/<target-path>`.
3. Skip anything in `ignore`.
4. Generate `envs/<name>/aenv.toml` with `[adapters.<name>] user_files = [...]` listing every target path, plus `[lifecycle]` if specified.
5. The convention file ITSELF is not copied into the namespace (it's a recipe, not content).

The spec lives in `pm_docs/aenv-namespace-toml-spec.md`. claude-ctrl is encouraged (in a follow-up upstream PR) to ship its own `aenv-namespace.toml`; until they do, the heuristic fallback handles their repo.

### Heuristic fallback (when `aenv-namespace.toml` is absent)

`import_global(fs, layout, adapters, source: &Path, name: &str) -> Result<()>`:
1. Validate name (no collision).
2. `source` must be a directory.
3. Look for `source/aenv-namespace.toml`. If present, use it (skip to step 5).
4. Otherwise, run the heuristic:
   - If `source/CLAUDE.md` exists, target `.claude/CLAUDE.md`.
   - If `source/AGENTS.md` exists, target `.codex/AGENTS.md`.
   - If `source/settings.json` exists, target `.claude/settings.json`.
   - If `source/agents/`, `source/commands/`, `source/hooks/`, `source/skills/` exist, target `.claude/agents/`, etc.
   - If `source/runtime/`, `source/bin/`, `source/sidecars/` exist, target `.claude/runtime/`, etc.
   - If `source/install.sh` exists, set `on_activate = "install.sh"` in the generated manifest.
   - If `source/uninstall.sh` exists, set `on_deactivate = "uninstall.sh"`.
5. Copy each detected/declared source into `envs/<name>/user/<target-path>`.
6. Write the manifest.

Document the heuristic AND the convention file in the import command's `--help` so users know both paths exist.

- [ ] **Step 2: CLI wiring**

```rust
    Import {
        /// Source path or git URL (URL handling in Task 6).
        source: String,
        /// Namespace name (defaults to last path component if omitted).
        name: Option<String>,
    },
```

- [ ] **Step 3: Tests**

```rust
#[test]
fn global_import_local_dir_produces_activable_namespace() {
    // Create a fake claude-ctrl-style source dir in tempdir.
    // import it.
    // assert envs/foo/user/.claude/CLAUDE.md exists with the right content.
    // assert the manifest declares all the imported paths.
    // Activate it. Confirm files materialize under fake_home.
}

#[test]
fn global_import_handles_partial_trees() {
    // Source has only CLAUDE.md and hooks/, no settings.json.
    // Import succeeds with a smaller user_files list.
}
```

- [ ] **Step 4: Commit**

```
Issue #4 followup: aenv global import <local-path> <name> (F9 part 1)
```

---

## Task 6: `aenv global import` git URL support (F9 part 2)

**Files:**
- Modify: `crates/aenv-core/src/global_snapshot.rs`
- Modify: `crates/aenv-cli/src/cmd/global/import.rs`
- Create: `crates/aenv-cli/tests/global_import_git_e2e.rs`

**Why:** `aenv global import https://github.com/juanandresgs/claude-ctrl claude-cntrl` is the headline UX. Reuses the same git-clone primitive as `aenv skill import git+URL`.

- [ ] **Step 1: Detect git URLs**

Heuristic: source starts with `https://`, `git://`, `git@`, or ends with `.git`. Anything else is a local path.

- [ ] **Step 2: Reuse git-clone primitive**

Look at `crates/aenv-core/src/skills/git*.rs` for the existing shell-out-to-git logic. Reuse it: shallow clone the URL into a tempdir, then call the local-path import logic on the cloned tree.

- [ ] **Step 3: Pin/ref support**

`--pin <ref>` to clone a specific tag/commit. `aenv skill import` already does this; mirror the flag.

- [ ] **Step 4: Tests**

Live network test marked `#[ignore]` (CI doesn't have network necessarily). Local-fixture version uses a git repo created in tempdir to validate the path without network.

- [ ] **Step 5: Commit**

```
Issue #4 followup: aenv global import git+URL (F9 part 2)
```

---

# Milestone K — Lifecycle hooks

## Task 7: `[lifecycle]` manifest schema

**Files:**
- Modify: `crates/aenv-core/src/manifest.rs`
- Modify: `crates/aenv-core/tests/manifest.rs`

**Why:** First step of F2. Schema-only change: lets a namespace declare `on_activate` + `on_deactivate` script paths. No execution yet; that's Task 8.

- [ ] **Step 1: Define the lifecycle struct**

```rust
/// Namespace lifecycle scripts. Paths are namespace-relative (under
/// `envs/<ns>/`); aenv runs them at the appropriate boundary with the
/// activation target root as the working directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LifecycleHooks {
    /// Script to run after files are materialized but before activation
    /// is declared successful. Failure rolls back materialization. Path
    /// is relative to the namespace directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_activate: Option<String>,
    /// Script to run before deactivation undoes any materialized files.
    /// Failure logs a warning but does not block deactivation (since the
    /// user is likely trying to recover from a broken state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_deactivate: Option<String>,
}
```

- [ ] **Step 2: Wire into `AenvManifest`**

```rust
    #[serde(default, skip_serializing_if = "LifecycleHooks::is_empty")]
    pub lifecycle: LifecycleHooks,
```

`impl LifecycleHooks { fn is_empty(&self) -> bool { self.on_activate.is_none() && self.on_deactivate.is_none() } }`

- [ ] **Step 3: Tests**

```rust
#[test]
fn manifest_lifecycle_roundtrip() {
    let toml = r#"
name = "ns"

[lifecycle]
on_activate = "install.sh"
on_deactivate = "uninstall.sh"
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.lifecycle.on_activate.as_deref(), Some("install.sh"));
    assert_eq!(m.lifecycle.on_deactivate.as_deref(), Some("uninstall.sh"));
}

#[test]
fn manifest_lifecycle_optional() {
    let m = AenvManifest::from_toml(r#"name = "ns""#).unwrap();
    assert!(m.lifecycle.on_activate.is_none());
}
```

- [ ] **Step 4: Validation**

Reject lifecycle paths that escape the namespace dir (absolute, `..` segments, `~/`). Same rules as `[[skills]].path` validation.

- [ ] **Step 5: Commit**

```
Issue #4 followup: namespace [lifecycle] manifest section (F2 part 1)
```

---

## Task 8: Activator runs `on_activate`; rolls back on failure

**Files:**
- Modify: `crates/aenv-core/src/activate/mod.rs`
- Create: `crates/aenv-core/tests/lifecycle_activate.rs`

**Why:** This is the load-bearing piece — without it, F2 is just metadata. claude-cntrl's `pip install` runs here.

- [ ] **Step 1: Add lifecycle execution after materialization**

In `activate_namespace_in_scope`, after the materialization loop and BEFORE writing the state file:

```rust
    // Lifecycle: on_activate runs with CWD = target_root, env includes
    // AENV_NAMESPACE + AENV_SCOPE + AENV_TARGET_ROOT. Script path is
    // namespace-relative; resolve against the namespace dir.
    let lifecycle_ran = if let Some(script) = leaf_manifest.lifecycle.on_activate.as_ref() {
        let script_path = layout.namespace_dir(leaf.as_str()).join(script);
        if !fs.exists(&script_path)? {
            return Err(AenvError::ManifestInvalid(format!(
                "on_activate '{}' does not exist in namespace dir", script
            )));
        }
        match run_lifecycle_script(&script_path, target_root, leaf, scope) {
            Ok(()) => true,
            Err(e) => {
                undo(fs, std::mem::take(&mut undo_log));
                return Err(AenvError::GlobalConflict(format!(
                    "on_activate failed: {e} — activation rolled back"
                )));
            }
        }
    } else {
        false
    };
```

`run_lifecycle_script` is a private helper that:
- Sets env: `AENV_NAMESPACE=<leaf>`, `AENV_SCOPE=<scope>`, `AENV_TARGET_ROOT=<target_root>`, `AENV_NAMESPACE_DIR=<ns_dir>`.
- Working dir: `target_root`.
- Streams stdout/stderr to inherit (so user sees `pip install` output).
- Returns Err on non-zero exit.

- [ ] **Step 2: Record `lifecycle_ran` on state**

Add `lifecycle_ran: bool` to `ActivationState` so deactivate knows whether to run `on_deactivate`.

- [ ] **Step 3: Tests**

```rust
#[test]
fn on_activate_success_runs_to_completion() {
    // Namespace with on_activate = "ok.sh" that exits 0.
    // Activate. Assert state file has lifecycle_ran=true.
    // Assert ok.sh actually ran (touched a sentinel file under target_root).
}

#[test]
fn on_activate_failure_rolls_back_materialization() {
    // Namespace with on_activate = "fail.sh" that exits 1.
    // Pre-existing user file at target.
    // Activate. Expect GlobalConflict.
    // Assert pre-existing file is intact (rollback restored it).
    // Assert no global-state.json was written.
}

#[test]
fn on_activate_missing_script_errors_with_manifest_invalid() {
    // Namespace declares on_activate = "ghost.sh" but ghost.sh doesn't exist.
    // Activate. Expect ManifestInvalid pre-flight.
}
```

- [ ] **Step 4: Commit**

```
Issue #4 followup: on_activate execution + rollback (F2 part 2)
```

---

## Task 9: Deactivator runs `on_deactivate` (best-effort)

**Files:**
- Modify: `crates/aenv-core/src/deactivate.rs`
- Modify: `crates/aenv-core/tests/lifecycle_activate.rs` (extend)

**Why:** Symmetric companion to Task 8. claude-cntrl's `pip uninstall` runs here. Best-effort because the user may be trying to recover from a broken state.

- [ ] **Step 1: Read manifest at deactivate time**

`deactivate_namespace_in_scope` reads the state file to get `active_namespace`. To run `on_deactivate`, also load the namespace's manifest. (Namespace might have been edited since activation — that's fine; we run the current version of the script.)

- [ ] **Step 2: Run on_deactivate before undoing files**

If `state.lifecycle_ran` is true AND the manifest's `on_deactivate` is set AND `--force` is NOT in effect:
```rust
    if let Err(e) = run_lifecycle_script(...) {
        eprintln!("warning: on_deactivate failed: {e}; continuing with file restoration");
    }
```

Note: best-effort. We do not abort deactivation on script failure.

- [ ] **Step 3: Tests**

```rust
#[test]
fn on_deactivate_runs_during_normal_deactivation() {
    // Namespace with on_deactivate = "cleanup.sh".
    // Activate + deactivate.
    // Assert cleanup.sh ran (sentinel file).
}

#[test]
fn on_deactivate_failure_does_not_block_file_restoration() {
    // Namespace with on_deactivate that exits 1.
    // Activate + deactivate.
    // Assert files are restored despite the script failure.
}
```

- [ ] **Step 4: Commit**

```
Issue #4 followup: on_deactivate execution (best-effort, F2 part 3)
```

---

## Task 10: First-activation approval prompt — namespace-scoped + script-SHA-pinned

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/activate.rs`
- Modify: `crates/aenv-cli/src/main.rs` (add `--yes` flag)
- Modify: `crates/aenv-cli/tests/global_activate_e2e.rs`

**Why:** `on_activate` runs arbitrary user-authored shell with the user's privileges. First time a user activates a namespace with lifecycle hooks, print the script path + the script's hash + the first lines of its content, and prompt for confirmation. `--yes` skips the prompt (CI / scripts). Re-approving on script content change is the load-bearing security property.

**Corrections from the original critique:**
- Approval is **namespace-scoped**, not user-wide. The marker lives at `~/.aenv/envs/<ns>/.approved` (a single file per namespace, content = the approved script's sha256). One file per namespace beats one global JSON list (which becomes another shared mutable state file).
- The marker stores the script's **sha256**, so re-editing the script invalidates the prior approval and re-prompts. Without this, a malicious or accidental edit to `on_activate.sh` after first approval would execute silently.

- [ ] **Step 1: Approval marker**

Per-namespace file at `<aenv_home>/envs/<ns>/.approved`. Content is a single line: `sha256:<hex>` matching the approved `on_activate` script content. (Stored under the namespace dir, not the registry root, so a namespace can be deleted and the approval goes with it.)

Helper:
```rust
fn approval_path(layout: &RegistryLayout, ns: &NamespaceId) -> PathBuf {
    layout.namespace_dir(ns.as_str()).join(".approved")
}

fn approval_status(layout: &RegistryLayout, ns: &NamespaceId, script: &Path) -> Result<ApprovalStatus> {
    // Returns:
    //   - Approved if .approved exists and its hash matches the current script bytes.
    //   - ScriptChanged if .approved exists but the hash differs.
    //   - NotApproved if .approved is absent.
    //   - NoScript if the namespace has no on_activate (no prompt needed).
}
```

- [ ] **Step 2: Prompt UX**

```
About to run on_activate hook for 'claude-cntrl':
  Script: /home/angel/.aenv/envs/claude-cntrl/install.sh
  sha256: 3a4f8b...c2d1
  First 8 lines:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "$AENV_NAMESPACE_DIR"
    pip install --user -e ./runtime
    echo "claude-ctrl runtime installed"
    …

Allow this script to run on every future activation until its content changes? [y/N]:
```

If the script previously had a different hash, the prompt says so:
```
The on_activate script for 'claude-cntrl' has changed since your last approval.
  Previously approved sha256: 3a4f8b...c2d1
  Current sha256:             7e2a1d...b3f0
  …content preview…
Re-approve? [y/N]:
```

- [ ] **Step 3: `--yes` flag**

`aenv global activate <ns> --yes` skips the prompt AND writes the approval marker so the next un-flagged activation is silent. `aenv use <ns> --global --yes` propagates.

`--yes` is documented as "I trust this namespace's lifecycle scripts (current and future)." For scripts changing across releases, automation should re-confirm — `--yes-once` could be a future addition; not in scope for v0.1.0.

- [ ] **Step 4: Tests**

```rust
#[test]
fn first_activation_with_on_activate_prompts_without_yes() {
    // Activate without --yes, no stdin piping. Expect command to error
    // (or hang — use a timeout) because there's no input.
}

#[test]
fn yes_flag_writes_approval_marker() {
    // Activate with --yes. Assert .approved file exists with correct sha256.
}

#[test]
fn second_activation_with_unchanged_script_does_not_prompt() {
    // Activate with --yes. Deactivate. Activate again WITHOUT --yes,
    // no stdin. Should succeed (approved from previous run).
}

#[test]
fn script_change_invalidates_approval() {
    // Activate with --yes. Deactivate. Edit on_activate script content.
    // Activate again WITHOUT --yes. Should prompt (or error in our test setup).
}

#[test]
fn approval_marker_serialization_round_trip() {
    // Unit test for the marker file shape.
}
```

- [ ] **Step 5: Commit**

```
Issue #4 followup: first-activation approval prompt — namespace-scoped, script-SHA-pinned (F2 part 4)
```

---

## Task 11: Document the lifecycle contract for namespace authors

**Files:**
- Modify: `README.md`
- Create: `pm_docs/lifecycle-hooks.md`

**Why:** Namespace authors need to know what env vars are set, what CWD is, what exit semantics are, what to do on failure. Document once, link everywhere. The honest framing of the failure model matters here: aenv can roll back the files it materialized, but it cannot undo arbitrary side effects of the script (a partial `pip install`, a touched system file, a spawned daemon). Tell authors that explicitly.

- [ ] **Step 1: Write the spec**

Required sections (all of these, not "some of"):

**Execution timing**
- `on_activate` runs AFTER files are materialized into `target_root` and BEFORE the state file is written. This means: if `on_activate` fails, aenv rolls back the materialization. If aenv writes the state file successfully, `on_activate` ran successfully.
- `on_deactivate` runs BEFORE files are undone, while the activation is still semantically live.

**Environment**
- `AENV_NAMESPACE` — the namespace name (e.g. `claude-cntrl`).
- `AENV_SCOPE` — `project` or `user`.
- `AENV_TARGET_ROOT` — absolute path to the activation target (project root or `$HOME`).
- `AENV_NAMESPACE_DIR` — absolute path to the namespace's source dir (e.g. `~/.aenv/envs/claude-cntrl/`).
- `AENV_LIFECYCLE_EVENT` — `activate` or `deactivate`.
- `AENV_FORCE` — set to `1` when `--force` is in effect for deactivation (script SHOULD short-circuit gracefully).

**Working directory**
- `$AENV_TARGET_ROOT`. Relative paths from scripts resolve under `$HOME` for user scope and under the project for project scope.

**Exit codes**
- 0 = success. Activation continues; deactivation continues.
- Non-zero on `on_activate` = activation rolls back files and returns `GlobalConflict` (exit 19).
- Non-zero on `on_deactivate` = aenv logs a warning to stderr but continues with file restoration. The user is likely trying to recover from a bad state; aenv prioritizes file restoration over script success.
- `on_deactivate` is NOT run when `--force` is passed.

**REQUIRED invariants (these are not suggestions)**

Authors MUST:
- **Be idempotent.** `on_activate` may run multiple times in succession (after a rollback, after an unclean previous run, after a script-change re-approval). It must produce the same final state regardless of how many times it runs from the same starting state. (`pip install --user -e .` is idempotent; `rm -rf $HOME/foo` is not.)
- **Be deterministic on failure.** A non-zero exit must leave the system in either the pre-script state (rollback-safe) or a state that the next idempotent run will heal. Half-baked state that NEITHER `on_activate` re-running NOR `on_deactivate` can fix is the worst-case footgun.

Authors MUST NOT:
- Modify `<aenv_home>/global-state.json`, `<aenv_home>/global.lock`, anything under `<aenv_home>/global-stash/`, or any file under `<aenv_home>/envs/<other-ns>/`. These are aenv's state; touching them corrupts the activation model.
- Remove or replace any of the symlinks/files aenv materialized this activation. The undo log assumes those are aenv-managed; tampering with them breaks rollback.
- Spawn long-running background processes without recording their PIDs in the namespace's own state. If your script `nohup`s a daemon, your `on_deactivate` must kill it; otherwise deactivation leaves orphan processes.
- Assume any specific shell. Hashbang your script (`#!/usr/bin/env bash`); aenv does not modify your shebang.

**Rollback semantics**
- aenv's undo log restores **only files aenv touched.** If your `on_activate` ran `pip install`, the Python package stays installed even after rollback. If your `on_activate` wrote `~/.foorc`, that file stays unless your script also tracked it. Author responsibility.
- The optional `aenv-rescue` binary (Task 14) does NOT run `on_deactivate`. Lifecycle scripts may depend on the very runtime that's broken; running them is exactly what locks users out. `aenv-rescue` is the "I don't trust my own lifecycle scripts to clean up" path.

**Approval model**
- First activation prompts (Task 10). `--yes` skips and writes the approval marker.
- Approval is per-namespace and pinned to the script's sha256. Editing the script invalidates the prior approval.

- [ ] **Step 2: Commit**

---

## Task 12: Wire a mini lifecycle fixture into the test suite

**Files:**
- Create: `crates/aenv-cli/tests/lifecycle_mini_fixture.rs`

**Why:** End-to-end proof that an `on_activate` hook running `pip install` against a real `pyproject.toml`-style source tree works. Use a tempdir as `$HOME` AND as the namespace dir. Use a minimal Python package as the install target (not real claude-ctrl runtime — too heavy for fast CI; covered by Task 12.5).

- [ ] **Step 1: Build the fixture**

Tempdir contains a minimal `pyproject.toml` (empty package called `aenv-test-pkg`) and an `install.sh` that runs `pip install --user -e .`.

- [ ] **Step 2: Activate the fixture, assert install ran**

After activate, `python3 -c "import aenv_test_pkg"` succeeds. After deactivate, the package is uninstalled.

This test requires Python + pip available. Mark `#[ignore]` if CI doesn't have them; run locally.

- [ ] **Step 3: Commit**

```
Issue #4 followup: lifecycle e2e mini fixture — F2 part 5
```

---

## Task 12.5: Real `claude-ctrl` integration test (`#[ignore]` by default)

**Files:**
- Create: `crates/aenv-cli/tests/lifecycle_claude_ctrl_real.rs`

**Why:** The mini fixture in Task 12 proves the machinery; Task 12.5 proves it against the *actual* target the user asked us to support. Without this, "works on a toy" doesn't equal "works on claude-ctrl." Marked `#[ignore]` so it doesn't run on every `cargo test` — invoke with `cargo test -- --ignored` or `cargo test --test lifecycle_claude_ctrl_real -- --ignored` locally before each release.

- [ ] **Step 1: Test scaffolding**

```rust
//! Real claude-ctrl integration. Clones the upstream repo, imports it as
//! a namespace, activates it under a tempdir-HOME with --yes, asserts the
//! key markers are in place (hooks materialized, pip-installed runtime
//! importable), then deactivates and asserts restoration.
//!
//! This test:
//! - clones from the public github repo (network required)
//! - runs `pip install --user` (Python + pip required)
//! - is slow (~30s on a warm machine)
//!
//! Hence the `#[ignore]` — gate on a CI label or run pre-release locally.

#[test]
#[ignore = "real-network + pip required; run with --ignored before release"]
fn claude_ctrl_imports_activates_deactivates_clean() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();

    // 1. Snapshot the empty fake_home as "default" baseline.
    let out = aenv()
        .env("AENV_HOME", &aenv_home).env("HOME", &fake_home)
        .args(["global", "snapshot", "default"])
        .output().unwrap();
    assert!(out.status.success(), "snapshot failed: {}",
        String::from_utf8_lossy(&out.stderr));

    // 2. Import claude-ctrl from upstream.
    let out = aenv()
        .env("AENV_HOME", &aenv_home).env("HOME", &fake_home)
        .args(["global", "import",
               "https://github.com/juanandresgs/claude-ctrl",
               "claude-cntrl"])
        .output().unwrap();
    assert!(out.status.success(), "import failed: {}",
        String::from_utf8_lossy(&out.stderr));

    // 3. Activate with --yes so the lifecycle approval prompt doesn't block.
    let out = aenv()
        .env("AENV_HOME", &aenv_home).env("HOME", &fake_home)
        .args(["global", "activate", "claude-cntrl", "--yes"])
        .output().unwrap();
    assert!(out.status.success(), "activate failed: {}",
        String::from_utf8_lossy(&out.stderr));

    // 4. Spot-check materialization markers.
    assert!(fake_home.join(".claude/CLAUDE.md").exists());
    assert!(fake_home.join(".claude/settings.json").exists());
    assert!(fake_home.join(".claude/hooks/pre-bash.sh").exists());

    // 5. If pip install ran, runtime should resolve. (Conditional on the
    // claude-ctrl repo's actual layout; adjust the import name to whatever
    // pyproject.toml declares at test-write time.)
    let py = std::process::Command::new("python3")
        .args(["-c", "import sys; sys.exit(0)"])  // placeholder — substitute the real import
        .output().unwrap();
    assert!(py.status.success(), "runtime not importable post-activate");

    // 6. Swap to default.
    let out = aenv()
        .env("AENV_HOME", &aenv_home).env("HOME", &fake_home)
        .args(["global", "activate", "default", "--yes"])
        .output().unwrap();
    assert!(out.status.success(), "swap-back failed: {}",
        String::from_utf8_lossy(&out.stderr));

    // 7. Deactivate. Original (empty) state should return.
    aenv().env("AENV_HOME", &aenv_home).env("HOME", &fake_home)
        .args(["global", "deactivate"]).status().unwrap();
    assert!(!fake_home.join(".claude/CLAUDE.md").exists()
        || std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap().is_empty());
}
```

- [ ] **Step 2: Document the pre-release ritual**

Add to `RELEASING.md`: *"Before tagging, run `cargo test -p aenv-cli --test lifecycle_claude_ctrl_real -- --ignored`. The test fetches the live claude-ctrl repo and exercises snapshot → import → activate → deactivate end-to-end."*

- [ ] **Step 3: Commit**

```
Issue #4 followup: real claude-ctrl integration test (gated #[ignore])
```

---

# Milestone L — Recovery

## Task 13: `aenv global deactivate --force`

**Files:**
- Modify: `crates/aenv-cli/src/main.rs`
- Modify: `crates/aenv-cli/src/cmd/global/deactivate.rs`
- Modify: `crates/aenv-cli/tests/global_activate_e2e.rs`

**Why:** When a broken `on_deactivate` would itself fail (because the runtime it depends on is missing), `--force` skips the lifecycle script and just undoes files.

- [ ] **Step 1: Flag**

```rust
    Deactivate {
        /// Skip on_deactivate. Use when the lifecycle hook itself is broken
        /// (e.g. it depends on a runtime that's missing or corrupted).
        #[arg(long)]
        force: bool,
        /// (existing) Also remove orphan stash directories.
        #[arg(long)]
        prune: bool,
    },
```

- [ ] **Step 2: Plumbing**

`cmd::global::deactivate::run` takes `force: bool` and passes it through. The core `deactivate_namespace_in_scope` accepts a new `skip_lifecycle: bool` parameter (default false — project-side call from `aenv deactivate` still works).

- [ ] **Step 3: Test**

```rust
#[test]
fn global_deactivate_force_skips_failing_on_deactivate() {
    // Namespace with on_deactivate that always fails.
    // Activate (with --yes).
    // Deactivate WITHOUT --force: still succeeds (best-effort), but emits a warning.
    // Re-activate. Deactivate WITH --force: succeeds silently, no script execution.
}
```

- [ ] **Step 4: Commit**

```
Issue #4 followup: aenv global deactivate --force (F3 part 1)
```

---

## Task 14: `aenv-rescue` standalone Rust binary

**Files:**
- Create: `crates/aenv-rescue/Cargo.toml`
- Create: `crates/aenv-rescue/src/main.rs`
- Modify: `Cargo.toml` (workspace members)
- Modify: `README.md` (link from the recovery section)

**Why:** When Claude Code's hooks lock the user out, they need a tool they can run from a non-Claude shell that doesn't depend on the (broken) hook chain. **Correction from the original critique:** the first draft proposed a POSIX sh script with a jq-or-grep JSON parser fallback. The grep fallback is genuinely fragile — JSON layout changes between serde_json versions could silently break recovery — and worst-case fragility coincides with worst-case need. Ship a Rust binary instead: statically linked, no external dependencies (including no jq), parses state via the same serde model the main binary uses.

The new binary is tiny: it depends on `aenv-core` (for `RegistryLayout`, `ActivationState`) plus `std`. No clap, no MCP, no skill machinery. Total surface: ~150 lines.

- [ ] **Step 1: Workspace member**

In root `Cargo.toml`, add `crates/aenv-rescue` to `[workspace] members`.

`crates/aenv-rescue/Cargo.toml`:
```toml
[package]
name = "aenv-rescue"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "aenv-rescue"
path = "src/main.rs"

[dependencies]
aenv-core = { path = "../aenv-core" }
```

- [ ] **Step 2: Implementation**

`crates/aenv-rescue/src/main.rs`:

```rust
//! `aenv-rescue` — emergency deactivate when the active namespace's hooks
//! have locked the user out of their shell. No external dependencies; reads
//! the same state file the main binary writes, undoes activation via direct
//! filesystem operations, never spawns subprocesses (so user hooks don't
//! fire).
//!
//! Usage: aenv-rescue
//!
//! Reads $AENV_HOME (default $HOME/.aenv), opens $AENV_HOME/global-state.json,
//! removes every managed file, restores every backup, deletes the state file
//! and lock. Exits 0 on success; non-zero with a diagnostic on failure.

use std::path::PathBuf;
use std::process::ExitCode;

fn aenv_home() -> PathBuf {
    std::env::var("AENV_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME must be set");
            PathBuf::from(home).join(".aenv")
        })
}

fn main() -> ExitCode {
    let aenv_home = aenv_home();
    let state_path = aenv_home.join("global-state.json");
    if !state_path.exists() {
        println!("No active global activation.");
        return ExitCode::SUCCESS;
    }
    let body = match std::fs::read_to_string(&state_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("aenv-rescue: cannot read {}: {e}", state_path.display());
            return ExitCode::from(1);
        }
    };
    let state = match aenv_core::state::ActivationState::from_json(&body) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("aenv-rescue: state file is malformed: {e}");
            return ExitCode::from(1);
        }
    };

    let target_root = state.project_root.clone();
    println!(
        "Rescuing global activation of '{}' under {}",
        state.active_namespace,
        target_root.display()
    );

    // 1. Remove every managed file (symlink or copy).
    for m in &state.managed_files {
        let full = target_root.join(&m.path);
        match std::fs::symlink_metadata(&full) {
            Ok(meta) if meta.file_type().is_dir() => {
                let _ = std::fs::remove_dir_all(&full);
            }
            Ok(_) => {
                let _ = std::fs::remove_file(&full);
            }
            Err(_) => { /* already gone */ }
        }
    }

    // 2. Restore every backup.
    for b in &state.backed_up {
        let original = target_root.join(&b.original_path);
        if b.backup_path.exists() {
            if let Some(parent) = original.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if original.exists() {
                let _ = std::fs::remove_file(&original);
            }
            if let Err(e) = std::fs::rename(&b.backup_path, &original) {
                eprintln!(
                    "aenv-rescue: could not restore {}: {e}",
                    original.display()
                );
            }
        }
    }

    // 3. Tear down state + lock.
    let _ = std::fs::remove_file(&state_path);
    let _ = std::fs::remove_file(aenv_home.join("global.lock"));

    // 4. NOTE: aenv-rescue does NOT run on_deactivate. Lifecycle scripts may
    // depend on the very runtime that's broken; running them is what locked
    // the user out in the first place.
    println!("Rescue complete. Run `aenv global status` to confirm.");
    ExitCode::SUCCESS
}
```

- [ ] **Step 3: Tests**

Create `crates/aenv-rescue/tests/rescue_e2e.rs`. Use `assert_cmd`-style invocation via `env!("CARGO_BIN_EXE_aenv-rescue")`:

```rust
#[test]
fn rescue_restores_after_simulated_lockout() {
    // Set up an active state by calling activate via aenv binary or by
    // constructing the state + stash manually. Then invoke aenv-rescue.
    // Assert: state file gone, lock gone, original files restored.
}

#[test]
fn rescue_with_no_active_state_is_noop() {
    // Empty $AENV_HOME. aenv-rescue exits 0 with "No active global activation."
}

#[test]
fn rescue_does_not_run_on_deactivate() {
    // Active state with on_deactivate that touches a sentinel file.
    // aenv-rescue. Sentinel should NOT be touched.
}
```

- [ ] **Step 4: README + walkthrough**

In the "Recovery" subsection of the README (added in Task 23): *"If your shell is locked out by an active namespace's hooks, run `aenv-rescue` from any shell that isn't going through Claude Code. It uses the state file as ground truth, undoes the activation via direct filesystem operations, and never invokes lifecycle scripts."*

- [ ] **Step 5: Commit**

```
Issue #4 followup: aenv-rescue Rust binary for hook-lockout recovery (F3 part 2)
```

---

## Task 15: Document the recovery flow end-to-end

**Files:**
- Modify: `pm_docs/walkthrough-global-namespaces.md`

**Why:** Make the recovery story discoverable. A new "When things go wrong" section in the walkthrough showing both `--force` and `aenv-rescue` paths, when to use which.

- [ ] **Step 1: Write the section**

Cover:
- Symptom: every Bash call returns a hook error.
- First try: open a new terminal (not via Claude Code), run `aenv global deactivate --force`.
- If that fails: run `bash /path/to/aenv-rescue` from any shell.
- After recovery: investigate what broke (probably an `on_activate` left the env in a bad state), fix the namespace, try again.

- [ ] **Step 2: Commit**

---

# Milestone M — Pre-flight safety

## Task 16: Settings.json command-path scanner

**Files:**
- Create: `crates/aenv-core/src/preflight.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/preflight.rs`

**Why:** Catches "namespace declares hooks/servers referencing files that don't exist" before activation succeeds and locks the user out. The demo lockout was exactly this case — claude-ctrl's `pre-bash.sh` calls `python3 ~/.claude/runtime/cli.py`, the cli.py didn't exist, fail-closed hook denies every subsequent shell call.

**Correction from the original critique:** The first draft only scanned `.hooks.*.[].hooks[].command`. Real settings.json files have several other places where a binary/script path can appear and a missing target produces the same lockout class. Scan all of them.

- [ ] **Step 1: Define the JSON paths to scan**

A `SettingsRef` is a `(json-pointer, kind, command-string)` triple. For each Claude Code settings.json candidate, extract every reference at these JSON pointers:

| Pointer | Kind | Notes |
|---|---|---|
| `/hooks/<event>/[]/hooks/[]/command` | `Hook(event)` | event ∈ {SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, WorktreeCreate, …}. Already shipped in the first draft. |
| `/mcpServers/<name>/command` | `McpServer(name)` | An MCP server with a missing command produces a broken server registration; Claude Code may also refuse to start sessions. |
| `/statusLine/command` | `StatusLine` | The statusLine command runs continuously; missing path means a broken UI element on every prompt. |
| `/awsCredentialExport/command` | `CredentialExport` | Documented optional in Claude Code's settings. Same fail-closed nature. |
| `/apiKeyHelper` | `ApiKeyHelper` | A string command. |
| `/stop_advisor` (and similar advisor entries Claude Code may add) | `Advisor` | Forward-compat: if Claude Code grows new top-level command-shaped settings, the scanner should be easy to extend. |

Implementation: a static `KNOWN_COMMAND_POINTERS: &[(JsonPointer, fn(&str) -> Kind)]` table. New entries are one-line additions.

- [ ] **Step 2: API**

```rust
pub struct PreflightFinding {
    pub settings_path: PathBuf,
    pub kind: PreflightKind,
    pub command: String,
    pub missing_path: PathBuf,
}

pub enum PreflightKind {
    Hook { event: String },
    McpServer { name: String },
    StatusLine,
    CredentialExport,
    ApiKeyHelper,
    Advisor { name: String },
}

pub fn preflight_settings_commands<F: Filesystem>(
    fs: &F,
    target_root: &Path,
    candidates: &[Candidate],
) -> Result<Vec<PreflightFinding>>
```

For each settings.json candidate:
1. Read content.
2. Parse as `serde_json::Value`.
3. Walk every entry in `KNOWN_COMMAND_POINTERS`.
4. For each found command string, resolve `$HOME` and `$AENV_TARGET_ROOT` env-var-style references against `target_root`. Extract argv[0] (split on first whitespace; preserve quoting if leading char is `"` or `'`).
5. Skip commands that are bare binary names (resolved via `$PATH`); only flag absolute paths or `~/`-rooted paths.
6. Check existence. If missing, emit a `PreflightFinding`.

- [ ] **Step 3: Skip-list for paths being materialized this run**

The pre-flight finding is suppressed if the referenced absolute path lies inside `target_root` AND is materialized by one of the candidates in the same activation. (Otherwise the very namespace bootstrapping itself would always trip.)

- [ ] **Step 4: Tests**

```rust
#[test]
fn preflight_flags_missing_hook_command() { /* hooks.PreToolUse */ }
#[test]
fn preflight_flags_missing_mcp_server() { /* mcpServers.foo.command */ }
#[test]
fn preflight_flags_missing_status_line_command() { /* statusLine.command */ }
#[test]
fn preflight_skips_bare_binaries_in_PATH() { /* e.g. "python3" alone */ }
#[test]
fn preflight_skips_paths_being_materialized_this_run() { /* hook points at .claude/hooks/x.sh which is in user_files */ }
#[test]
fn preflight_handles_malformed_settings_json_gracefully() { /* parse error logged, not crashed */ }
```

- [ ] **Step 5: Commit**

```
Issue #4 followup: preflight settings.json command scanner (hooks, MCP, statusLine, etc.) — F5 part 1
```

---

## Task 17: Wire pre-flight into doctor

**Files:**
- Modify: `crates/aenv-core/src/doctor.rs`
- Modify: `crates/aenv-core/src/policies/builtin/`
- Modify: `crates/aenv-cli/src/cmd/global/doctor.rs`

**Why:** Surface preflight findings as policy outcomes so they appear in `aenv global doctor <ns>` output and the JSON shape.

- [ ] **Step 1: Add a synthetic policy**

`hook_paths_resolvable` — never declared by namespaces; auto-fires when doctor evaluates a namespace with a settings.json candidate. Emits Warn per missing path.

- [ ] **Step 2: Display**

```
[WARN] hook_paths_resolvable claude-cntrl::~/.claude/settings.json:
  PreToolUse hook references /home/angel/.claude/runtime/cli.py — does not exist.
  Hint: run the namespace's install procedure or declare runtime/ in user_files.
```

- [ ] **Step 3: Tests**

- [ ] **Step 4: Commit**

```
Issue #4 followup: doctor reports unresolvable hook paths (F8)
```

---

## Task 18: Pre-flight on activation (opt-out)

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/activate.rs`
- Modify: `crates/aenv-cli/src/main.rs`

**Why:** Run the preflight scanner automatically during `aenv global activate`. If any findings, print them as warnings and prompt to continue. `--skip-preflight` opt-out for power users.

- [ ] **Step 1: Run scanner during activate**

After resolving candidates but before materializing, call `preflight_settings_hooks`. If non-empty:
```
Pre-flight found 2 potential issues:
  - PreToolUse hook in .claude/settings.json references ~/.claude/runtime/cli.py (missing)
  - SessionStart hook in .claude/settings.json references ~/.claude/hooks/session-init.sh (will be materialized)

Continue? [y/N]:
```

(Only count truly-missing paths in the first line; paths that will be materialized by THIS activation are fine.)

- [ ] **Step 2: `--skip-preflight` and `--yes`**

`--yes` answers yes to both the lifecycle prompt and the preflight prompt. `--skip-preflight` skips the scan entirely.

- [ ] **Step 3: Tests**

- [ ] **Step 4: Commit**

```
Issue #4 followup: activation runs pre-flight scan + prompt (F5 part 2)
```

---

# Milestone N — Content hash in `which`

## Task 19: `aenv global which --json` includes content hash

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/which.rs`
- Modify: `crates/aenv-cli/tests/global_readonly_e2e.rs`

**Why:** F6. Lets external tooling (harness-eval) detect per-file drift without re-resolving. Same hash semantics as R-84 (sha256 of resolved bytes).

- [ ] **Step 1: Compute content hash**

For a found candidate, read its source bytes (or use the material-set bytes via `compute_material_set_user`), hash with SHA-256, prefix with `"sha256:"`.

- [ ] **Step 2: JSON shape**

```json
{
  "scope": "user",
  "path": "~/.claude/CLAUDE.md",
  "qualified": "claude-cntrl::.claude/CLAUDE.md",
  "strategy": "symlink",
  "content_hash": "sha256:abc123..."
}
```

- [ ] **Step 3: Tests**

Snapshot the shape with `insta` or a custom assertion. Verify the hash matches what `sha256sum` produces on the source file.

- [ ] **Step 4: Commit**

```
Issue #4 followup: aenv global which --json includes content_hash (F6)
```

---

# Milestone O — Copy strategy

## Task 20: `materialize = "copy"` adapter option

**Files:**
- Modify: `crates/aenv-core/src/adapter.rs`
- Modify: `crates/aenv-core/src/strategy.rs`
- Modify: `crates/aenv-core/tests/adapter.rs` + `tests/strategy.rs`

**Why:** Default symlink semantics mean editing `~/.claude/CLAUDE.md` writes through to the namespace source. Surprising for daily-driver users. Per-adapter opt-in to "copy" gets a regular file that's safe to edit in-place; on deactivate, the namespace source isn't modified, and aenv overwrites the user's edit on the next activation (documented behavior).

- [ ] **Step 1: Schema**

```rust
    /// Default materialization strategy for this adapter when a single
    /// candidate yields a symlink decision. `"symlink"` (default) creates
    /// a symlink to the source; `"copy"` writes a copy of the source bytes
    /// to the target. Copy mode trades the silent-edit-through-symlink
    /// gotcha for losing in-place edits on the next activation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialize: Option<String>,
```

Per-namespace override via manifest can stay as a `merge_override`-style mechanism, OR via a new adapter-entry field. Prefer adapter-level for now; per-file override can come later if needed.

- [ ] **Step 2: Strategy selection**

In `decide_strategy`, single-candidate branch: if `adapter.materialize == Some("copy")`, return `MaterializeStrategy::Copy` instead of `Symlink`.

- [ ] **Step 3: Materializer Copy path**

Today, `MaterializeStrategy::Copy` is a Phase-7-deferred variant that hard-errors in both `activate/mod.rs:363` and the deactivate code path. Implementing it is more surgery than "add a decision arm":

1. `activate/mod.rs::materialize_one` — replace the `MaterializeStrategy::Copy => Err(...)` arm with a real implementation: read source bytes via `fs.read()`, write to target via `fs.write()`, treat as Symlink for backup/displace semantics (i.e., the `Displaced` branch still backs up the original; the `Absent` branch creates parent dirs then writes).
2. `activate/phase1.rs::materialize_symlink` — currently only handles symlink case; add a parallel `materialize_copy` helper or thread the strategy through the existing function.
3. `materialize::compute_material_set_for_scope` — already handles Copy correctly in `materialize_one_in_memory` (reads source bytes). Verify it still does after step 1's refactor.
4. `deactivate.rs` — currently `MaterializeStrategy::Symlink | MaterializeStrategy::Copy => remove_file`. Already correct. Verify with a test.
5. `diff.rs` — drift detection for Copy needs to compare on-disk bytes to source bytes. Already byte-level for symlinks; Copy is symmetric.

**Estimate revision:** This task originally said ~1 day. Realistic estimate after the Copy-strategy audit: **~2 days** (including writing the new `materialize_copy` helper, ~6 test cases, and verifying drift detection still works).

- [ ] **Step 4: Deactivate Copy handling**

Same shape as symlink — remove the materialized file, restore backup if present. The existing `deactivate.rs` already covers `Copy` in the same match arm as `Symlink`, so this step is mostly verification + a fresh test that exercises a real Copy deactivation.

- [ ] **Step 5: Tests**

```rust
#[test]
fn copy_strategy_creates_regular_file_not_symlink() { /* read meta; assert file, not symlink */ }
#[test]
fn copy_strategy_writes_correct_bytes_to_target() { /* read target bytes; compare to source */ }
#[test]
fn copy_strategy_with_displaced_target_backs_up_original() { /* pre-existing file → backup row */ }
#[test]
fn copy_strategy_with_absent_target_records_was_present_false() { /* uses Task 2's flag */ }
#[test]
fn deactivate_copy_removes_file_and_restores_backup() { /* round trip */ }
#[test]
fn drift_diff_detects_local_edit_to_copy_target() { /* manual edit; aenv global diff reports it */ }
```

- [ ] **Step 6: Commit**

```
Issue #4 followup: implement materialize = "copy" end-to-end (F7 part 1)
```

---

## Task 21: Per-namespace `materialize` override

**Files:**
- Modify: `crates/aenv-core/src/manifest.rs`
- Modify: `crates/aenv-core/src/strategy.rs`

**Why:** A namespace can override the adapter default. Useful when one namespace (claude-cntrl, with binary runtime) wants symlinks for speed, and another (custom-tweaks, where user edits in-place) wants copy.

- [ ] **Step 1: Manifest extension**

```rust
    /// Override the adapter's default materialize strategy for this
    /// namespace. Same values as `Adapter::materialize`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialize: Option<String>,
```

On `AdapterEntry`.

- [ ] **Step 2: Strategy selection precedence**

Order: per-file `merge_override` > namespace `materialize` > adapter `materialize` > default Symlink.

- [ ] **Step 3: Tests**

- [ ] **Step 4: Commit**

```
Issue #4 followup: per-namespace materialize override (F7 part 2)
```

---

## Task 22: Doctor warns on copy-mode + recently-edited target

**Files:**
- Modify: `crates/aenv-core/src/preflight.rs` (or wherever the doctor scans extend)
- Modify: `crates/aenv-cli/src/cmd/global/doctor.rs`

**Why:** A user in copy mode might have edited `~/.claude/CLAUDE.md` since the last activation. On next activate, those edits get clobbered. Doctor warns: "you've edited a managed file in copy mode; reactivation will overwrite it. Snapshot or commit your changes first."

- [ ] **Step 1: Detection**

For each `Copy`-strategy managed file in the active state, compare current on-disk bytes against `state.managed_files[i].content_hash` (which we'd record at materialization time — needs schema bump? or just compute lazily).

Defer the content_hash-on-state-file decision: cheaper to compute lazily by reading source bytes. Use `compute_material_set_user` to get the expected bytes.

- [ ] **Step 2: Doctor outcome**

```
[WARN] copy_mode_edit_loss claude-cntrl::~/.claude/CLAUDE.md:
  Local edits detected since last activation. Re-activating will
  overwrite them. Run `aenv global snapshot <name>` first to capture.
```

- [ ] **Step 3: Commit**

```
Issue #4 followup: doctor warns when copy-mode targets have local edits (F7 part 3)
```

---

# Milestone P — Docs + release

## Task 23: README rewrite for global namespaces

**Files:**
- Modify: `README.md`

**Why:** The current section sells "one command swaps your harness." That isn't true today and won't be true after this plan lands either — but it'll be MUCH closer. Rewrite to be precise about what aenv does (file positions + lifecycle scripts) and what the user owns (anything not declared in `user_files`).

- [ ] **Step 1: Outline the new section**

Subheadings (all are REQUIRED — none of them is optional):
- **What it does** (1 paragraph, honest framing). aenv moves files; lifecycle scripts handle runtime dependencies; the user owns anything not declared.
- **The snapshot → import → swap → deactivate cycle** (the headline UX).
- **Lifecycle hooks: when to use them, what they do, security model**. Brief; link to `pm_docs/lifecycle-hooks.md` (Task 11) for the full contract.
- **Editing a live activation: where your edits go** — explicitly cover the symlink-vs-copy difference. With default symlink mode, editing `~/.claude/CLAUDE.md` while a namespace is active edits the namespace source through the symlink (`<aenv_home>/envs/<ns>/user/.claude/CLAUDE.md`). With `materialize = "copy"`, edits are local and get overwritten on the next activation of the same namespace. State both behaviors plainly; recommend `aenv global snapshot` before editing.
- **Recovery: `--force` and `aenv-rescue`**. When to use which.
- **Limitations: what's NOT in scope** (link to the "Out of scope" list — generic package manager, live-reload, process-tree isolation).
- **Pointer to the walkthrough**.

- [ ] **Step 2: Write it**

- [ ] **Step 3: Commit**

---

## Task 24: Walkthrough rewrite with claude-cntrl end-to-end

**Files:**
- Modify or rewrite: `pm_docs/walkthrough-global-namespaces.md`

**Why:** The current walkthrough doesn't reflect the snapshot → import → lifecycle flow. Rewrite to use the actual claude-ctrl repo as the worked example, including the install step.

- [ ] **Step 1: Outline**

1. `aenv global snapshot default` — capture user's current state.
2. `aenv global import https://github.com/juanandresgs/claude-ctrl claude-cntrl` — import as a namespace.
3. Hand-edit the manifest to add `[lifecycle] on_activate = "install.sh"` (or use the auto-detected version if Task 5's heuristic handles it).
4. `aenv global activate claude-cntrl --yes` — runs install.sh, materializes files. Show expected output (pip output, success line).
5. Use Claude Code with claude-cntrl active. Show one or two characteristic claude-cntrl features.
6. `aenv global activate default` — swap back, runs claude-cntrl's `on_deactivate`, materializes default's files.
7. Recovery section: simulate a broken `on_activate` (rename `install.sh` to break the script), activate, observe rollback. Then simulate a broken `on_deactivate`, recover with `--force`. Then simulate a locked-out shell, recover with `aenv-rescue`.

- [ ] **Step 2: Verify every command in the walkthrough**

Live test in a tempdir-as-HOME against a fresh aenv build. Output blocks should match what the binary actually emits.

- [ ] **Step 3: Commit**

---

## Task 25: CHANGELOG + version bump + tag

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml`

**Why:** Standard release hygiene. Bump to `0.1.0` — Issue #4 plus this follow-up plan together justify the minor bump.

- [ ] **Step 1: CHANGELOG entry**

Major sections:
- Added: snapshot, import, lifecycle hooks, force/rescue, preflight, content hash, copy mode.
- Changed: SCHEMA_VERSION 5 → 6.
- Documentation: README + walkthrough rewrite.

- [ ] **Step 2: Version bump**

Workspace `version = "0.0.3"` → `"0.1.0"`.

- [ ] **Step 3: Commit + tag**

```bash
git commit -m "Release: v0.1.0 — Issue #4 global namespaces, daily-driver complete"
git tag -a v0.1.0 -m "global namespaces with snapshot/import/lifecycle"
```

(Do not push; user pushes.)

- [ ] **Step 4: Close issue**

```bash
gh issue close 4 --comment "Shipped in v0.1.0. Snapshot, import, lifecycle, rescue, preflight, content-hash, copy-mode all in. See CHANGELOG and pm_docs/walkthrough-global-namespaces.md."
```

(Only on user's explicit go-ahead.)

---

## Task 26: Final cross-cutting verification

**Files:**
- Run: full workspace test, clippy, fmt.
- Verify: `aenv global --help` lists all new subcommands and flags.

**Why:** Pre-tag sanity check.

- [ ] **Step 1: Run**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --all --check
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace
```

Expected: all green.

- [ ] **Step 2: Manual smoke test against fake $HOME**

A fresh tempdir, snapshot, import a small fixture, activate with lifecycle, swap, deactivate. Confirm every step prints what the docs claim it prints.

- [ ] **Step 3: Sign-off**

---

# Stretch — post-MVP, file as follow-ups

These are not in the v0.1.0 MVP. They surfaced during critique as legitimate concerns that don't block the daily-driver experience but should be tracked. File each as its own GitHub issue once v0.1.0 ships.

## S1 — `aenv global update <ns>` (refresh imported namespace)

**Why:** When claude-ctrl ships a new commit, today the user has to delete + re-import the namespace. State file dangles; approval may re-prompt; user-side edits in the namespace dir get lost. A first-class update verb re-clones the source, copies new bytes (preserving user edits via three-way merge OR with explicit `--overwrite-mine` flag), and re-runs `on_activate` if currently active.

**Approximate cost:** ~1 day. Reuses import logic + Task 10's approval invalidation.

## S2 — Drift detection across both symlink and copy modes

**Why:** Task 22 detects local edits to copy-mode targets. Symlink mode has no analogous warning — if the user edits `~/.aenv/envs/claude-cntrl/user/.claude/CLAUDE.md` directly (because the symlink resolves there), they have uncommitted changes in the namespace source that may surprise them on the next swap. For git-tracked namespaces, `aenv global doctor` could run `git status` against the namespace dir and warn on uncommitted changes.

**Approximate cost:** ~half day. Pure doctor extension.

## S3 — `aenv global activate --resume`

**Why:** If `on_activate` is mid-pip-install and the user Ctrl-Cs, the activation is half-done: files materialized, state file written, lifecycle ran=false. Re-running activate today restarts the whole materialization (lock acquired, atomicity probed, files re-materialized — wasteful). `--resume` re-runs ONLY the lifecycle script against the existing materialization.

**Approximate cost:** ~half day. State-file extension + activator branch.

## S4 — `--yes-once` for one-shot CI

**Why:** `--yes` permanently approves the script. CI scripts that activate-then-deactivate per build don't want to leave a marker behind. `--yes-once` approves for this run only.

**Approximate cost:** ~half day. Single flag + branch in approval logic.

## S5 — Telemetry: which lifecycle scripts have run on this machine

**Why:** A user reactivating after a long gap might forget which namespaces have lifecycle hooks they've approved. `aenv global doctor` could list "approved lifecycle scripts" with last-activated dates. Mostly UX.

**Approximate cost:** ~half day. Reads existing per-namespace `.approved` files.

---

# Self-review checklist

- [ ] Every F1–F8 item from the scoping is covered:
  - F1 → Tasks 3, 4
  - F2 → Tasks 7, 8, 9, 10, 11, 12, 12.5
  - F3 → Tasks 13, 14, 15
  - F4 → Tasks 23, 24
  - F5 → Tasks 16, 17, 18
  - F6 → Task 19
  - F7 → Tasks 20, 21, 22
  - F8 → Task 17 (folded into doctor)
  - F9 → Tasks 5, 6 (import)
  - F10 → Task 1 (docs)
- [ ] Critique findings addressed:
  - Task 2's flag moved from `BackedUpFile` to `ManagedFile` (correctness fix — `Absent` paths have no backup row to hang the flag on).
  - Task 14 `aenv-rescue` is a Rust binary, not a POSIX sh script (removes jq dependency / grep-parser fragility).
  - Task 16 pre-flight scans MCP servers, statusLine, advisors, credential exports — not only `.hooks.*`.
  - Task 12.5 added: real claude-cntrl integration test (`#[ignore]`-gated, runs pre-release).
  - Task 10 approval is namespace-scoped (per-namespace `.approved` file, not a global JSON list) and pinned to the script's sha256 (re-prompts on edit).
  - Task 11 lifecycle contract enumerates REQUIRED invariants (idempotency, no state-file tampering, no orphan daemons, hashbang required), not just "guidance."
  - Task 20 estimate bumped from ~1 day to ~2 days after auditing `MaterializeStrategy::Copy` (currently a Phase-7-deferred hard-error; real implementation touches more than just the strategy decider).
  - F11-style drift detection added explicitly as Stretch S2 so it's not silently dropped.
- [ ] No placeholders. Each task either has code blocks or has a precise scope (the test-body sketches are intentional — implementing agent fills them in concretely, same convention as the original Issue #4 plan).
- [ ] Type consistency: `LifecycleHooks` introduced in Task 7 is read by Tasks 8, 9, 10, 22. `ActivationState.lifecycle_ran` added in Task 8 is read by Tasks 9, 13. `BackedUpFile.was_present_before_activation` added in Task 2 is read by Tasks 9, 22.
- [ ] Public-contract changes summarized in the header.
- [ ] Out-of-scope list inherited from Issue #4 + the new generic-package-manager exclusion.

---

# Execution

Once approved, kick off via `superpowers:subagent-driven-development`. Dispatch one subagent per task or per tight pair (e.g. Tasks 5+6 together since both are "import"). Two-stage review (spec + code quality) after each.

**Estimated total: ~10 working days.**
