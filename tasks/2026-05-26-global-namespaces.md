# Global Namespaces (Issue #4) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> Project precedent: `cargo` is not on PATH. Every `cargo` invocation must be prefixed with `PATH="$HOME/.cargo/bin:$PATH"`. Project style: `rustfmt max_width = 100`, `clippy -D warnings`. Commit small and often; the existing repo commits directly to `main` for feature work and uses annotated tags at completion (`global-namespaces-complete` upon landing).

**Goal:** Add an `aenv global …` subcommand tree that materializes a namespace's user-scoped files into `$HOME` (e.g. `~/.claude/`, `~/.codex/`) using the same stash + symlink + merge machinery used for project-scoped activation, so one namespace switch swaps hooks, agents, commands, and user instructions in a single transaction.

**Architecture:** The existing activation pipeline (resolver → strategy → merge → materialize → state) is already path-generic — almost every primitive takes a `project_root: &Path`. We introduce a `Scope` enum and route both the *target root* (project vs. `$HOME`) and the *manifest filter* (project files vs. user files) through it. Resolution stays one-pass; we partition candidates by declared scope. A new top-level CLI subcommand `aenv global` mirrors the existing project verbs. A new state file at `$HOME/.aenv/global-state.json`, a stash at `$HOME/.aenv/global-stash/<ts>/`, and a lock at `$HOME/.aenv/global.lock` provide isolation from the per-project state.

**Tech stack:** Same as the rest of the workspace — Rust, `clap` v4 derive, `serde` + `toml`, `serde_json`, `thiserror`, `tempfile` + `insta` + `proptest` for tests. Shells out to no new tooling.

**Public contracts touched:**
- New exit code **19 — global activation conflict** (orphan stash, concurrent activation, schema mismatch).
- Adapter TOML schema gains optional `user_files`, `user_roles`, `user_default_merge`, `user_merge_strategies`, `user_soft_limits`, `user_skills_dir`. All default-empty; nothing breaks for adapters that don't declare them.
- Namespace manifest `[adapters.<name>]` block gains optional `user_files` array and per-file `user_merge` map. `[[skills]]` table gains optional `scope = "user" | "project"` (default `"project"`).
- Namespace on-disk layout gains a sibling `envs/<ns>/user/` subtree for user-scoped sources. Project-scoped sources still live at `envs/<ns>/<path>`.
- Resolved-namespace hash (R-84) is computed **per scope**. The project-scope hash is unchanged (existing namespaces with no user content hash identically before and after this feature). A new `hash_resolved_namespace_user` exists for the user-scope side.
- JSON output schemas: existing `aenv status --json`, `aenv which --json`, etc. unchanged. New `aenv global status --json`, `aenv global doctor --json`, `aenv global which --json`, `aenv global diff --json` schemas added under `insta` snapshots.

**Sequencing principle:** Each task produces compilable, testable software. Earlier tasks can land independently; later tasks build on the schema once it stabilizes. Tests are written before code for every behavioral change (TDD).

---

## Milestone groups

- **A. Schema** — Tasks 1–4. Adapter TOML, namespace manifest, scope enum, builtin adapter updates. Defensive parse + round-trip tests only; no activation yet.
- **B. Resolution** — Tasks 5–6. Path-generic resolver that filters by scope; user-scope soft-limit validation.
- **C. Global activation core** — Tasks 7–10. `activate_in_scope`, `deactivate_in_scope`, stash layout, state file, undo log retargeted to `$HOME`.
- **D. Locking + stale recovery** — Tasks 11–12. `global.lock` semantics.
- **E. CLI surface** — Tasks 13–18. `aenv global { use, deactivate, status, which, list, doctor, diff }` + the `--global` sugar.
- **F. Doctor + drift + orphan stash** — Tasks 19–20.
- **G. End-to-end + builtin coverage** — Tasks 21–23. Live `$HOME`-redirected tests + the seven builtin adapters' `user_files`.
- **H. Docs + walkthrough + release** — Tasks 24–25.

---

# Milestone A — Schema

## Task 1: Add `Scope` enum to `aenv-core`

**Files:**
- Create: `crates/aenv-core/src/scope.rs`
- Modify: `crates/aenv-core/src/lib.rs` (register the new module)
- Modify: `crates/aenv-core/tests/scope.rs` (created in this task)

**Why:** Every subsequent module that needs to know "am I dealing with project files or user files" routes through this enum. Defining it in isolation, with serde round-trip + a tiny set of behavioral tests, makes later integrations a one-line `match`.

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/scope.rs`:

```rust
use aenv_core::scope::Scope;

#[test]
fn scope_default_is_project() {
    assert_eq!(Scope::default(), Scope::Project);
}

#[test]
fn scope_serializes_as_lowercase() {
    let s = serde_json::to_string(&Scope::User).unwrap();
    assert_eq!(s, "\"user\"");
    let s = serde_json::to_string(&Scope::Project).unwrap();
    assert_eq!(s, "\"project\"");
}

#[test]
fn scope_deserializes_lowercase() {
    let s: Scope = serde_json::from_str("\"user\"").unwrap();
    assert_eq!(s, Scope::User);
    let s: Scope = serde_json::from_str("\"project\"").unwrap();
    assert_eq!(s, Scope::Project);
}

#[test]
fn scope_unknown_value_rejected() {
    let r: Result<Scope, _> = serde_json::from_str("\"system\"");
    assert!(r.is_err());
}

#[test]
fn scope_as_str_is_stable() {
    assert_eq!(Scope::Project.as_str(), "project");
    assert_eq!(Scope::User.as_str(), "user");
}
```

- [ ] **Step 2: Run test to verify it fails to compile**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --no-run`
Expected: error[E0433] cannot find `scope` in `aenv_core`.

- [ ] **Step 3: Write the module**

Create `crates/aenv-core/src/scope.rs`:

```rust
//! Activation scope: project-local (`<project>/`) vs user-global (`$HOME/`).
//!
//! Every materialization primitive that previously took a `project_root: &Path`
//! is now parameterized by a scope. The scope determines both the target root
//! (where files land) and the filter applied to namespace manifests (which
//! adapter file list is consulted: `files` vs `user_files`).

use serde::{Deserialize, Serialize};

/// Activation scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Project-local activation: files materialize under `<project_root>/`.
    /// State at `<project_root>/.aenv-state/state.json`.
    #[default]
    Project,
    /// User-global activation: files materialize under `$HOME/`.
    /// State at `$HOME/.aenv/global-state.json`.
    User,
}

impl Scope {
    /// Stable lowercase identifier for diagnostics and JSON output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Project => "project",
            Scope::User => "user",
        }
    }
}
```

Modify `crates/aenv-core/src/lib.rs` — add `pub mod scope;` to the alphabetized list (between `restore` and `shadow` or wherever fits).

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test scope`
Expected: 5 tests pass.

- [ ] **Step 5: Lint + format**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/scope.rs crates/aenv-core/src/lib.rs crates/aenv-core/tests/scope.rs
git commit -m "Issue #4: add Scope enum (Project|User) for global activation"
```

---

## Task 2: Extend `Adapter` struct with user-scope fields

**Files:**
- Modify: `crates/aenv-core/src/adapter.rs`
- Modify: `crates/aenv-core/tests/adapter.rs`

**Why:** The adapter TOML is the bridge between "this is a target file path under `$HOME`" and "this is a target file path under `<project>/`." The Adapter struct stores both sets without disturbing the existing API: callers that ignore user fields see no behavioral change.

- [ ] **Step 1: Write the failing test**

Append to `crates/aenv-core/tests/adapter.rs`:

```rust
#[test]
fn adapter_user_files_round_trip() {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md", ".claude/"]
user_files = ["~/.claude/CLAUDE.md", "~/.claude/agents/", "~/.claude/settings.json"]
user_skills_dir = "~/.claude/skills"

[user_roles]
"~/.claude/CLAUDE.md" = "instructions"

[user_soft_limits]
instructions = 5000

[user_default_merge]
"~/.claude/settings.json" = "deep"
"#;
    let a = aenv_core::adapter::Adapter::from_toml(toml).unwrap();
    assert_eq!(a.user_files, vec![
        "~/.claude/CLAUDE.md".to_string(),
        "~/.claude/agents/".to_string(),
        "~/.claude/settings.json".to_string(),
    ]);
    assert_eq!(a.user_skills_dir.as_deref(), Some("~/.claude/skills"));
    assert_eq!(a.user_roles.get("~/.claude/CLAUDE.md").map(String::as_str), Some("instructions"));
    assert_eq!(a.user_soft_limits.get("instructions").copied(), Some(5000));
    assert_eq!(a.user_default_merge.get("~/.claude/settings.json").map(String::as_str), Some("deep"));
}

#[test]
fn adapter_without_user_fields_still_parses() {
    let toml = r#"
name = "cline"
files = [".clinerules"]
"#;
    let a = aenv_core::adapter::Adapter::from_toml(toml).unwrap();
    assert!(a.user_files.is_empty());
    assert!(a.user_roles.is_empty());
    assert!(a.user_soft_limits.is_empty());
    assert!(a.user_default_merge.is_empty());
    assert!(a.user_skills_dir.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test adapter -- adapter_user_files_round_trip adapter_without_user_fields_still_parses`
Expected: compilation error on `user_files` (field doesn't exist).

- [ ] **Step 3: Extend the struct**

In `crates/aenv-core/src/adapter.rs`, add to `Adapter` (between `soft_limits` and the close brace):

```rust
    /// User-scope analog of `files`. Paths typically start with `~/` and are
    /// expanded against `$HOME` at activation time. Empty when the adapter has
    /// no user-level surface.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_files: Vec<String>,
    /// User-scope analog of `roles`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub user_roles: BTreeMap<String, String>,
    /// User-scope analog of `default_merge`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub user_default_merge: BTreeMap<String, String>,
    /// User-scope analog of `merge_strategies` (Phase-1 holdover; kept for symmetry).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub user_merge_strategies: BTreeMap<String, String>,
    /// User-scope analog of `soft_limits`. Same role-keyed shape.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub user_soft_limits: BTreeMap<String, usize>,
    /// User-scope analog of `skills_dir`. For claude-code this is `~/.claude/skills`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_skills_dir: Option<String>,
```

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test adapter`
Expected: all existing tests still pass; new two pass.

- [ ] **Step 5: Lint**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/adapter.rs crates/aenv-core/tests/adapter.rs
git commit -m "Issue #4: extend Adapter with user_files/roles/limits/skills_dir"
```

---

## Task 3: Extend manifest `AdapterEntry` with `user_files`

**Files:**
- Modify: `crates/aenv-core/src/manifest.rs`
- Modify: `crates/aenv-core/tests/manifest.rs`

**Why:** Namespace authors opt their files into the user scope here. The wire shape mirrors `files` exactly — a string array plus an optional `user_merge` map — so authors can copy the pattern they already know.

- [ ] **Step 1: Write the failing test**

Append to `crates/aenv-core/tests/manifest.rs`:

```rust
#[test]
fn manifest_user_files_round_trip() {
    let toml = r#"
name = "research"

[adapters.claude-code]
files = ["CLAUDE.md"]
user_files = [".claude/CLAUDE.md", ".claude/agents/code-reviewer.md", ".claude/settings.json"]
user_merge = { ".claude/settings.json" = "deep" }
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    let entry = m.adapters.get("claude-code").expect("adapter present");
    assert_eq!(entry.user_files, vec![
        ".claude/CLAUDE.md".to_string(),
        ".claude/agents/code-reviewer.md".to_string(),
        ".claude/settings.json".to_string(),
    ]);
    let user_merge = entry.user_merge.as_ref().expect("user_merge present");
    assert_eq!(user_merge.get(".claude/settings.json").map(String::as_str), Some("deep"));
}

#[test]
fn manifest_user_files_optional() {
    // Existing-shape manifest with no user_files must still parse cleanly.
    let toml = r#"
name = "legacy"

[adapters.claude-code]
files = ["CLAUDE.md"]
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    let entry = m.adapters.get("claude-code").unwrap();
    assert!(entry.user_files.is_empty());
    assert!(entry.user_merge.is_none());
}

#[test]
fn manifest_user_files_uniform_merge() {
    // Bare-string `user_merge = "deep"` expands to per-file map.
    let toml = r#"
name = "uniform"

[adapters.claude-code]
user_files = [".claude/a.json", ".claude/b.json"]
user_merge = "deep"
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    let entry = m.adapters.get("claude-code").unwrap();
    let user_merge = entry.user_merge.as_ref().unwrap();
    assert_eq!(user_merge.get(".claude/a.json").map(String::as_str), Some("deep"));
    assert_eq!(user_merge.get(".claude/b.json").map(String::as_str), Some("deep"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test manifest -- manifest_user_files`
Expected: compilation error — `user_files` field missing on `AdapterEntry`.

- [ ] **Step 3: Extend `AdapterEntry` and its raw parser**

In `crates/aenv-core/src/manifest.rs`:

(a) Add to the `AdapterEntry` struct (between `merge` and the close brace):

```rust
    /// User-scope analog of `files`. Paths are relative to the namespace
    /// `user/` source subdir and to `$HOME` at activation time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_files: Vec<String>,
    /// User-scope analog of `merge`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_merge: Option<std::collections::BTreeMap<String, String>>,
```

(b) In `AenvManifest::from_toml`, extend `RawAdapterEntry`:

```rust
        struct RawAdapterEntry {
            #[serde(default)]
            files: Vec<String>,
            #[serde(default)]
            merge: Option<MergeRaw>,
            #[serde(default)]
            user_files: Vec<String>,
            #[serde(default)]
            user_merge: Option<MergeRaw>,
        }
```

(c) In the Stage-2 expansion (where `merge` becomes per-file), repeat the same shape for `user_merge` and emit it on the `AdapterEntry`:

```rust
                let RawAdapterEntry { files, merge: merge_raw, user_files, user_merge: user_merge_raw } = raw_entry;
                let merge = merge_raw.map(|m| match m {
                    MergeRaw::PerFile(map) => map,
                    MergeRaw::Uniform(s) => files.iter().map(|f| (f.clone(), s.clone())).collect(),
                });
                let user_merge = user_merge_raw.map(|m| match m {
                    MergeRaw::PerFile(map) => map,
                    MergeRaw::Uniform(s) => user_files.iter().map(|f| (f.clone(), s.clone())).collect(),
                });
                (name, AdapterEntry { files, merge, user_files, user_merge })
```

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test manifest`
Expected: all existing pass; the three new pass.

- [ ] **Step 5: Lint**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/manifest.rs crates/aenv-core/tests/manifest.rs
git commit -m "Issue #4: manifest AdapterEntry gains user_files + user_merge"
```

---

## Task 4: Tag `[[skills]]` entries with scope

**Files:**
- Modify: `crates/aenv-core/src/skills/mod.rs` (or wherever `SkillDecl` lives — verify path before editing)
- Modify: `crates/aenv-core/tests/manifest_skills.rs`

**Why:** A skill declared `scope = "user"` materializes under `$HOME/.claude/skills/<name>/`, not `<project>/.claude/skills/<name>/`. Default omitted/`"project"` for backward compatibility.

- [ ] **Step 1: Verify location of `SkillDecl`**

Run: `grep -rn "struct SkillDecl" crates/aenv-core/src/`
Expected: one match, probably in `crates/aenv-core/src/skills/mod.rs`. Note the exact file before proceeding.

- [ ] **Step 2: Write the failing test**

Append to `crates/aenv-core/tests/manifest_skills.rs`:

```rust
#[test]
fn skill_scope_defaults_to_project() {
    let toml = r#"
name = "ns"
[[skills]]
name = "code-reviewer"
adapter = "claude-code"
mode = "authored"
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills[0].scope, aenv_core::scope::Scope::Project);
}

#[test]
fn skill_scope_user_round_trips() {
    let toml = r#"
name = "ns"
[[skills]]
name = "personal-helper"
adapter = "claude-code"
mode = "authored"
scope = "user"
"#;
    let m = aenv_core::manifest::AenvManifest::from_toml(toml).unwrap();
    assert_eq!(m.skills[0].scope, aenv_core::scope::Scope::User);
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test manifest_skills -- skill_scope`
Expected: compilation error — `scope` field missing on `SkillDecl`.

- [ ] **Step 4: Add `scope` to `SkillDecl`**

In the file `SkillDecl` lives in, add (immediately before the `path` field or similar — pick a stable position):

```rust
    /// Activation scope. Defaults to `Project`; `User` materializes under
    /// the adapter's `user_skills_dir` (e.g. `~/.claude/skills/<name>/`).
    #[serde(default)]
    pub scope: crate::scope::Scope,
```

- [ ] **Step 5: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test manifest_skills`
Expected: all pass.

- [ ] **Step 6: Lint**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/aenv-core/src/skills/mod.rs crates/aenv-core/tests/manifest_skills.rs
git commit -m "Issue #4: SkillDecl gains scope field (Project default | User)"
```

---

# Milestone B — Resolution

## Task 5: Path-generic resolver — partition candidates by scope

**Files:**
- Modify: `crates/aenv-core/src/resolve.rs`
- Modify: `crates/aenv-core/tests/composition.rs`

**Why:** This is the only non-trivial resolver change. Today, `gather_candidates` walks every adapter entry's `files` and emits `Candidate`s. We now also walk `user_files` and tag each candidate with its scope. The caller filters by scope to drive either project or user activation.

- [ ] **Step 1: Add `scope` field to `Candidate`**

In `crates/aenv-core/src/resolve.rs`, on the `Candidate` struct, add:

```rust
    /// Activation scope this candidate belongs to.
    pub scope: crate::scope::Scope,
```

(Place it right after `adapter`.)

Every existing site that constructs a `Candidate` must initialize this field. There are at most two: inside `gather_candidates`, inside `gather_skill_candidates`. Update both to `scope: crate::scope::Scope::Project,` for the existing code path.

- [ ] **Step 2: Write the failing test**

Append to `crates/aenv-core/tests/composition.rs`:

```rust
#[test]
fn resolver_emits_user_scope_candidates() {
    use aenv_core::scope::Scope;
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;

    // Adapter that declares user_files.
    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
files = ["CLAUDE.md"]
user_files = ["~/.claude/CLAUDE.md", "~/.claude/agents/"]
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    // Namespace with both scopes.
    let ns_dir = registry.namespace_dir("foo");
    std::fs::create_dir_all(ns_dir.join("user/.claude/agents")).unwrap();
    std::fs::write(ns_dir.join("CLAUDE.md"), b"project").unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"user").unwrap();
    std::fs::write(ns_dir.join("user/.claude/agents/reviewer.md"), b"agent").unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "foo"
[adapters.claude-code]
files = ["CLAUDE.md"]
user_files = [".claude/CLAUDE.md", ".claude/agents/reviewer.md"]
"#).unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("foo").unwrap();
    let result = aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let project_paths: Vec<_> = result.candidates.iter()
        .filter(|c| c.scope == Scope::Project)
        .map(|c| c.path.to_string_lossy().into_owned())
        .collect();
    let user_paths: Vec<_> = result.candidates.iter()
        .filter(|c| c.scope == Scope::User)
        .map(|c| c.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(project_paths, vec!["CLAUDE.md".to_string()]);
    assert_eq!(user_paths, vec![
        ".claude/CLAUDE.md".to_string(),
        ".claude/agents/reviewer.md".to_string(),
    ]);
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test composition -- resolver_emits_user_scope_candidates`
Expected: empty `user_paths` (user files not gathered yet).

- [ ] **Step 4: Extend `gather_candidates`**

In `crates/aenv-core/src/resolve.rs`, after the existing `for rel in &entry.files { … }` loop in `gather_candidates`, add a symmetric loop for `entry.user_files`:

```rust
        for rel in &entry.user_files {
            if rel.contains('*') {
                expand_glob(fs, &ns_root.join("user"), rel)
                    .map_err(|e| ResolutionError::Io(e.to_string()))?
                    .into_iter()
                    .for_each(|literal| {
                        out.push(Candidate {
                            namespace: ns.clone(),
                            path: PathBuf::from(&literal),
                            source_path: ns_root.join("user").join(&literal),
                            adapter: adapter_name.clone(),
                            merge_override: entry
                                .user_merge
                                .as_ref()
                                .and_then(|m| m.get(&literal).cloned()),
                            skill_provenance: None,
                            scope: crate::scope::Scope::User,
                        })
                    });
            } else {
                out.push(Candidate {
                    namespace: ns.clone(),
                    path: PathBuf::from(rel),
                    source_path: ns_root.join("user").join(rel),
                    adapter: adapter_name.clone(),
                    merge_override: entry.user_merge.as_ref().and_then(|m| m.get(rel).cloned()),
                    skill_provenance: None,
                    scope: crate::scope::Scope::User,
                });
            }
        }
```

Note: `source_path` includes the `user/` subdir under the namespace root. This is the **only** asymmetry between project and user — at the source side, they live in different subtrees so the same relative path can mean two different files.

- [ ] **Step 5: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test composition`
Expected: all existing pass; new test passes.

- [ ] **Step 6: Validation — reject user candidate paths that escape**

In the existing `validate_candidate_paths` function in the same file, reject any user-scope candidate whose path is absolute or starts with `~/` (the `~/` belongs in the adapter, not the namespace). Add to the existing checks (in the same `for c in candidates { … }` loop, after the `..` check):

```rust
        if c.scope == crate::scope::Scope::User && s.starts_with("~/") {
            return Err(ResolutionError::ManifestInvalid {
                namespace: c.namespace.clone(),
                reason: format!(
                    "user-scope candidate path begins with '~/': {} \
                     (drop the prefix; expansion happens at activation time)",
                    p.display()
                ),
            });
        }
```

(`s.starts_with("~/")` works against the existing `let s = p.to_string_lossy();` line.)

- [ ] **Step 7: Add a test for the escape check**

Append to the same test file:

```rust
#[test]
fn user_scope_path_with_tilde_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();
    let ns_dir = registry.namespace_dir("bad");
    std::fs::create_dir_all(&ns_dir).unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "bad"
[adapters.claude-code]
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let leaf = aenv_core::identity::NamespaceId::new("bad").unwrap();
    let err = aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.contains("~/"), "expected '~/' rejection, got {msg}");
}
```

- [ ] **Step 8: Run + lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test composition
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: resolver tags candidates with Scope; user_files gathered"
```

---

## Task 6: Doctor — user-scope soft-limit & forbidden-path policies

**Files:**
- Modify: `crates/aenv-core/src/doctor.rs`
- Modify: `crates/aenv-core/src/policies/builtin/` (whichever submodule owns instructions-soft-limit and forbid-paths)
- Modify: `crates/aenv-core/tests/doctor.rs`

**Why:** `aenv doctor` today reports instructions over the soft limit and any forbidden-path violations. Both checks must consult the right adapter list — `roles`+`soft_limits` for project scope, `user_roles`+`user_soft_limits` for user scope — and label outcomes with the scope so users see `[user] ~/.claude/CLAUDE.md` rather than just `CLAUDE.md`.

- [ ] **Step 1: Read the current implementations**

Run: `grep -n "soft_limits\|forbid_paths" crates/aenv-core/src/policies/builtin/*.rs crates/aenv-core/src/doctor.rs | head -40`

Note the lookup pattern these use against `Adapter`. The change is mechanical: also walk the `user_*` field for any candidate whose `scope == User`.

- [ ] **Step 2: Write the failing test**

Append to `crates/aenv-core/tests/doctor.rs`:

```rust
#[test]
fn doctor_reports_user_scope_soft_limit_violation() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    // Tiny soft limit on the user side; project side untouched.
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]

[user_roles]
"~/.claude/CLAUDE.md" = "instructions"

[user_soft_limits]
instructions = 10
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("oversize");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), "x".repeat(500)).unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "oversize"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#).unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("oversize").unwrap();
    let resolution = aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution);
    let labels: Vec<_> = report.outcomes.iter().map(|o| {
        let t = o.target.as_ref().map_or(String::new(), |t| t.to_string());
        format!("{} {}", o.key, t)
    }).collect();
    assert!(
        labels.iter().any(|l| l.contains("instructions-max-chars") && l.contains("~/.claude/CLAUDE.md")),
        "no user-scope soft-limit violation reported: {labels:?}"
    );
}
```

- [ ] **Step 3: Run it to see it fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test doctor -- doctor_reports_user_scope_soft_limit_violation`
Expected: violation not reported (because the policy only consults `adapter.roles` / `adapter.soft_limits`).

- [ ] **Step 4: Teach the soft-limit policy about scope**

Inside the soft-limits evaluator: when iterating candidates, branch on `candidate.scope`:
- `Scope::Project`: look up `role` in `adapter.roles`, limit in `adapter.soft_limits` (existing behavior).
- `Scope::User`: look up `role` in `adapter.user_roles`, limit in `adapter.user_soft_limits`.

The outcome label should be the *candidate path* (already includes its target dir), prefixed with `~/` when scope is `User`. Use a small helper:

```rust
fn target_label(c: &crate::resolve::Candidate) -> String {
    match c.scope {
        crate::scope::Scope::Project => c.path.to_string_lossy().into_owned(),
        crate::scope::Scope::User => format!("~/{}", c.path.display()),
    }
}
```

- [ ] **Step 5: Apply the same change to forbid-paths**

In `forbid_paths` (and any other path-scoped policy in `policies/builtin/`), prefer `target_label` over the bare `c.path`. The check itself is path-string; the label is for the human-facing report.

- [ ] **Step 6: Run all doctor tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test doctor --test policy_instructions_max_chars --test policy_forbid_paths`
Expected: all pass.

- [ ] **Step 7: Lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: doctor recognizes user-scope candidates for soft-limits + forbid-paths"
```

---

# Milestone C — Global activation core

## Task 7: `Scope`-aware activation entry point

**Files:**
- Modify: `crates/aenv-core/src/activate/mod.rs`
- Create: `crates/aenv-core/tests/activate_global_unit.rs`

**Why:** The existing `activate_namespace` is the project entry point. We add `activate_namespace_in_scope(fs, layout, adapters, target_root, scope, leaf)`. Project keeps its current public signature as a thin wrapper for back-compat; user is a new call that the CLI invokes for `aenv global use`.

- [ ] **Step 1: Sketch the new signature**

Plan-only step — no code yet. Read `crates/aenv-core/src/activate/mod.rs` and find `pub fn activate_namespace`. Note the three steps inside it that need scope routing:
1. `probe_rename_atomicity(fs, project_root)` → still atomicity-probes whichever root we're targeting.
2. `crate::resolve::resolve_namespace(...)` → returns *all* candidates; we filter by scope before grouping.
3. `let state_path = project_root.join(".aenv-state/state.json");` → differs per scope.

- [ ] **Step 2: Write the failing test**

Create `crates/aenv-core/tests/activate_global_unit.rs`:

```rust
//! Unit tests for activation in `Scope::User`. Uses a tempdir as fake `$HOME`
//! so we do NOT touch the developer's real `~/.claude`.

use std::path::Path;

#[test]
fn activate_user_scope_writes_files_under_home_and_state_under_aenv_home() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().to_path_buf();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md", "~/.claude/agents/"]
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("research");
    std::fs::create_dir_all(ns_dir.join("user/.claude/agents")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"# Research mode").unwrap();
    std::fs::write(ns_dir.join("user/.claude/agents/explorer.md"), b"explorer body").unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "research"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md", ".claude/agents/explorer.md"]
"#).unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("research").unwrap();
    let state = aenv_core::activate::activate_namespace_in_scope(
        &fs, &registry, &adapters, &fake_home, aenv_core::scope::Scope::User, &leaf,
    ).unwrap();

    assert_eq!(state.scope, aenv_core::scope::Scope::User);
    assert_eq!(state.active_namespace, "research");

    // Files materialized under fake_home (NOT under aenv_home or the namespace dir).
    let claude_md = fake_home.join(".claude/CLAUDE.md");
    let agent = fake_home.join(".claude/agents/explorer.md");
    assert!(claude_md.exists(), "CLAUDE.md should be materialized under $HOME");
    assert!(agent.exists(), "agent should be materialized under $HOME");

    // State file under aenv_home, not under fake_home root.
    let state_path = aenv_home.join("global-state.json");
    assert!(state_path.exists(), "global-state.json should be at $AENV_HOME/global-state.json");
    let body = std::fs::read_to_string(&state_path).unwrap();
    assert!(body.contains("\"scope\":\"user\""), "state file must record scope");

    // Project file (CLAUDE.md at fake_home root) is NOT materialized.
    let stray = fake_home.join("CLAUDE.md");
    assert!(!stray.exists(), "project-scope CLAUDE.md must not appear in user activation");

    // Cleanup
    let _ = std::fs::remove_dir_all(&fake_home.join(".claude"));
}
```

- [ ] **Step 3: Run it to see it fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate_global_unit`
Expected: compilation error — `activate_namespace_in_scope` doesn't exist; `state.scope` field doesn't exist.

- [ ] **Step 4: Extend `ActivationState` with `scope`**

In `crates/aenv-core/src/state.rs`, add to `ActivationState`:

```rust
    /// Activation scope this state file describes. Always `Project` for the
    /// legacy `<project>/.aenv-state/state.json` (default on read for schema-1..4).
    #[serde(default)]
    pub scope: crate::scope::Scope,
```

Bump `SCHEMA_VERSION` from `4` to `5`. Update the deserializer to also read `scope` with `#[serde(default)]` — older files default to `Project`, which preserves all existing behavior.

Update both `activate_namespace` callsites of `ActivationState { … }` to set `scope: crate::scope::Scope::Project`.

- [ ] **Step 5: Refactor `activate_namespace` → `activate_namespace_in_scope`**

In `crates/aenv-core/src/activate/mod.rs`:

```rust
/// Activate `leaf` namespace into `target_root` under `scope`. Resolves the
/// full `extends` chain, filters candidates to `scope`, then materializes.
///
/// For `Scope::Project`, `target_root` is the project root and state is written
/// to `<target_root>/.aenv-state/state.json`. For `Scope::User`, `target_root`
/// is `$HOME` and state is written to `<layout.root()>/global-state.json`.
pub fn activate_namespace_in_scope<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    target_root: &Path,
    scope: crate::scope::Scope,
    leaf: &NamespaceId,
) -> Result<ActivationState> {
    probe_rename_atomicity_for_scope(fs, layout, target_root, scope)?;

    let mut resolution = crate::resolve::resolve_namespace(fs, layout, adapters, leaf)?;
    // Filter candidates by scope.
    resolution.candidates.retain(|c| c.scope == scope);

    // … same as before, except:
    //   - backup_root computed from scope (see Task 8)
    //   - target_root replaces project_root in materialization helpers
    //   - state_path is computed from scope
    // …

    let state_path = match scope {
        crate::scope::Scope::Project => target_root.join(".aenv-state/state.json"),
        crate::scope::Scope::User => layout.global_state_path(),
    };
    let state = ActivationState {
        schema_version: SCHEMA_VERSION,
        scope,
        active_namespace: leaf.as_str().to_owned(),
        project_root: target_root.to_path_buf(),
        managed_files: managed,
        backed_up,
        parameters: resolved_parameters,
        policies: resolved_policies,
        warnings: resolution_warnings,
    };
    // … write logic identical
}

/// Backward-compatible wrapper. Schedules deletion in a future major; today it
/// just calls `activate_namespace_in_scope` with `Scope::Project`.
pub fn activate_namespace<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    leaf: &NamespaceId,
) -> Result<ActivationState> {
    activate_namespace_in_scope(fs, layout, adapters, project_root, crate::scope::Scope::Project, leaf)
}
```

- [ ] **Step 6: Add `RegistryLayout::global_state_path` and `global_stash_dir`**

In `crates/aenv-core/src/home.rs`, append:

```rust
    /// Path to the user-scope activation state file.
    pub fn global_state_path(&self) -> PathBuf {
        self.root.join("global-state.json")
    }

    /// Root of the user-scope stash directory; per-run stashes go under
    /// `<this>/<timestamp>/`.
    pub fn global_stash_root(&self) -> PathBuf {
        self.root.join("global-stash")
    }

    /// Path to the user-scope lock file.
    pub fn global_lock_path(&self) -> PathBuf {
        self.root.join("global.lock")
    }
```

- [ ] **Step 7: Atomicity probe — scope-aware location**

The existing probe writes `.probe.a` and `.probe.b` to `<project>/.aenv-state/`. For user scope we need it to write somewhere on the same filesystem as `$HOME` — `<aenv_home>/` is on the same filesystem as `$HOME` in the common case (both under `$HOME`), but a user could have set `AENV_HOME` to a different mount. Probe `target_root` itself, not the state-file directory:

```rust
fn probe_rename_atomicity_for_scope<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    target_root: &Path,
    scope: crate::scope::Scope,
) -> Result<()> {
    let probe_dir = match scope {
        crate::scope::Scope::Project => target_root.join(".aenv-state"),
        crate::scope::Scope::User => {
            // Stash root lives next to global-state.json; probing there ensures
            // backup renames are atomic. The activation target is $HOME; the user
            // is responsible for $HOME and AENV_HOME being on the same fs.
            layout.global_stash_root()
        }
    };
    crate::atomicity::probe_rename_atomicity_at(fs, &probe_dir)
}
```

…which means we also lift `probe_rename_atomicity` to take an arbitrary directory (rename existing to `probe_rename_atomicity_at` and keep a thin `probe_rename_atomicity` wrapper for the existing call site).

- [ ] **Step 8: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate_global_unit`
Expected: pass.

Re-run all existing activation tests to make sure the refactor didn't break anything:

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate --test composition --test deactivate --test fork`
Expected: all pass.

- [ ] **Step 9: Lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: activate_namespace_in_scope (Scope::User materializes under \$HOME)"
```

---

## Task 8: User-scope backup stash + retargeted materializer

**Files:**
- Modify: `crates/aenv-core/src/activate/mod.rs`
- Modify: `crates/aenv-core/src/activate/phase1.rs`
- Modify: `crates/aenv-core/tests/activate_global_unit.rs` (extend with stash test)

**Why:** Today `backup_dir_for_this_run(project_root)` returns `<project>/.aenv-state/backup/<ts>/`. For user scope the issue specifies `~/.aenv/global-stash/<ts>/`. We add a scope-aware variant. Materializer helpers (`materialize_symlink`, `write_merged_regular`) take a backup root parameter already, so retargeting is a one-call change.

- [ ] **Step 1: Write the failing test**

Append to `crates/aenv-core/tests/activate_global_unit.rs`:

```rust
#[test]
fn user_scope_stashes_displaced_files_under_aenv_home() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().to_path_buf();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    // Preexisting ~/.claude/CLAUDE.md that aenv must stash.
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original user CLAUDE.md").unwrap();

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new user CLAUDE.md").unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#).unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let state = aenv_core::activate::activate_namespace_in_scope(
        &fs, &registry, &adapters, &fake_home, aenv_core::scope::Scope::User, &leaf,
    ).unwrap();

    // Stash root must be under aenv_home, not fake_home.
    assert_eq!(state.backed_up.len(), 1);
    let stash = &state.backed_up[0].backup_path;
    assert!(stash.starts_with(&aenv_home), "stash {stash:?} must live under aenv_home");
    // Backup contents must be the original bytes.
    let body = std::fs::read(stash).unwrap();
    assert_eq!(body, b"original user CLAUDE.md");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate_global_unit -- user_scope_stashes`
Expected: failure (either the stash is under `fake_home`, or empty).

- [ ] **Step 3: Add a scope-aware backup-dir helper**

In `crates/aenv-core/src/activate/mod.rs`, replace `backup_dir_for_this_run` with a variant that knows about scope:

```rust
pub(super) fn backup_dir_for_this_run_in_scope(
    layout: &RegistryLayout,
    target_root: &Path,
    scope: crate::scope::Scope,
) -> PathBuf {
    let parent = match scope {
        crate::scope::Scope::Project => target_root.join(".aenv-state/backup"),
        crate::scope::Scope::User => layout.global_stash_root(),
    };
    parent.join(backup_timestamp())
}
```

Update the single call site (in `activate_namespace_in_scope`) to use the new function. Also update `backed_up: BackedUpFile { original_path, backup_path }` so `original_path` stays *relative to target_root* (consistent with how project state files store paths today). Verify by reading the `BackedUpFile` shape — `original_path` is project-relative; we keep that invariant per-scope (path is target_root-relative).

- [ ] **Step 4: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate_global_unit -- user_scope_stashes`
Expected: pass.

Re-run the full activation test surface:

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate --test deactivate`
Expected: all pass.

- [ ] **Step 5: Lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: scope-aware backup stash root (\$HOME/.aenv/global-stash/<ts>/)"
```

---

## Task 9: `deactivate_namespace_in_scope` — restore from global stash

**Files:**
- Modify: `crates/aenv-core/src/deactivate.rs`
- Create: `crates/aenv-core/tests/deactivate_global_unit.rs`

**Why:** Mirror of Task 7. Loads `global-state.json`, removes materialized files under `$HOME`, restores backed-up originals from the global stash, deletes the state file.

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/deactivate_global_unit.rs`:

```rust
#[test]
fn deactivate_user_scope_restores_stash_and_removes_state() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().to_path_buf();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original").unwrap();

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new").unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#).unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    aenv_core::activate::activate_namespace_in_scope(
        &fs, &registry, &adapters, &fake_home, aenv_core::scope::Scope::User, &leaf,
    ).unwrap();

    // Deactivate
    let active = aenv_core::deactivate::deactivate_namespace_in_scope(
        &fs, &registry, &fake_home, aenv_core::scope::Scope::User,
    ).unwrap();
    assert_eq!(active, "ns");
    // Original bytes restored.
    let restored = std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap();
    assert_eq!(restored, b"original");
    // State file gone.
    assert!(!aenv_home.join("global-state.json").exists());
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test deactivate_global_unit`
Expected: function doesn't exist.

- [ ] **Step 3: Implement the function**

In `crates/aenv-core/src/deactivate.rs`, add (and refactor the existing `deactivate_namespace` to delegate to it):

```rust
/// Scope-aware deactivation. For `Scope::Project` this matches the historical
/// behavior. For `Scope::User`, reads `<aenv_home>/global-state.json` and
/// restores backups stashed under `<aenv_home>/global-stash/<ts>/`.
pub fn deactivate_namespace_in_scope<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    target_root: &Path,
    scope: crate::scope::Scope,
) -> Result<String> {
    let state_path = match scope {
        crate::scope::Scope::Project => target_root.join(".aenv-state/state.json"),
        crate::scope::Scope::User => layout.global_state_path(),
    };
    // … remainder mirrors deactivate_namespace, but `project_root.join(&file.path)`
    // becomes `target_root.join(&file.path)`. The stash dir for prune purposes is
    // skipped — stashes are timestamped and auto-orphan; doctor handles cleanup.
}

pub fn deactivate_namespace<F: Filesystem>(fs: &F, project_root: &Path) -> Result<String> {
    // Re-create a transient layout from project; it's only used to compute
    // the state path which we override. For Project scope, `layout` is unused
    // by `deactivate_namespace_in_scope` past the state-path branch — but we
    // do need *a* layout. Use a placeholder rooted at project_root.
    let layout = RegistryLayout::new(project_root.to_path_buf());
    deactivate_namespace_in_scope(fs, &layout, project_root, crate::scope::Scope::Project)
}
```

(Where the unused-layout shape is ugly, prefer to *not* take `layout` for project and compute the state path inline. Two small functions are fine.)

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test deactivate --test deactivate_global_unit`
Expected: all pass.

- [ ] **Step 5: Lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: deactivate_namespace_in_scope restores user-scope stash"
```

---

## Task 10: Swap path — `aenv global use foo` after `aenv global use bar`

**Files:**
- Modify: `crates/aenv-core/src/activate/mod.rs`
- Modify: `crates/aenv-core/tests/activate_global_unit.rs`

**Why:** Project-scope today reuses `aenv use`'s "if pinned, replace pin; activation is a separate command" model — multiple project activations don't directly swap. For user scope, **one** activation lives system-wide, and `aenv global use bar` while `foo` is active means: deactivate `foo`, then activate `bar`, in one atomic flow. On `bar`'s failure, `foo` is reactivated.

- [ ] **Step 1: Write the failing test**

Append to `crates/aenv-core/tests/activate_global_unit.rs`:

```rust
#[test]
fn user_scope_swap_transactional() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().to_path_buf();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original").unwrap();

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    // Two namespaces.
    for (name, body) in [("foo", b"foo body" as &[u8]), ("bar", b"bar body")] {
        let ns_dir = registry.namespace_dir(name);
        std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
        std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), body).unwrap();
        std::fs::write(ns_dir.join("aenv.toml"), format!(r#"
name = "{name}"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#)).unwrap();
    }

    let foo = aenv_core::identity::NamespaceId::new("foo").unwrap();
    let bar = aenv_core::identity::NamespaceId::new("bar").unwrap();

    // Activate foo.
    aenv_core::activate::swap_or_activate_user(
        &fs, &registry, &adapters, &fake_home, &foo,
    ).unwrap();
    assert_eq!(std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(), b"foo body");

    // Swap to bar.
    aenv_core::activate::swap_or_activate_user(
        &fs, &registry, &adapters, &fake_home, &bar,
    ).unwrap();
    assert_eq!(std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(), b"bar body");

    // Deactivate restores the *original* (not foo's body) — only one level of stash matters.
    aenv_core::deactivate::deactivate_namespace_in_scope(
        &fs, &registry, &fake_home, aenv_core::scope::Scope::User,
    ).unwrap();
    assert_eq!(std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(), b"original");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate_global_unit -- user_scope_swap`
Expected: `swap_or_activate_user` doesn't exist.

- [ ] **Step 3: Implement the swap helper**

In `crates/aenv-core/src/activate/mod.rs`:

```rust
/// User-scope "activate, replacing any prior global activation" flow.
///
/// 1. If `<aenv_home>/global-state.json` exists, deactivate first.
/// 2. Activate `leaf`.
/// 3. On step-2 failure, attempt to re-activate the prior namespace as
///    rollback. If rollback fails, surface both errors.
pub fn swap_or_activate_user<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    target_root: &Path,
    leaf: &NamespaceId,
) -> Result<ActivationState> {
    let state_path = layout.global_state_path();
    let prior: Option<String> = if fs.exists(&state_path)? {
        let bytes = fs.read(&state_path)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;
        let state = ActivationState::from_json(text)?;
        crate::deactivate::deactivate_namespace_in_scope(
            fs, layout, target_root, crate::scope::Scope::User,
        )?;
        Some(state.active_namespace)
    } else {
        None
    };

    match activate_namespace_in_scope(
        fs, layout, adapters, target_root, crate::scope::Scope::User, leaf,
    ) {
        Ok(s) => Ok(s),
        Err(e) => {
            if let Some(prior_name) = prior {
                if let Ok(prior_id) = NamespaceId::new(prior_name.as_str()) {
                    let _ = activate_namespace_in_scope(
                        fs, layout, adapters, target_root, crate::scope::Scope::User, &prior_id,
                    );
                }
            }
            Err(e)
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate_global_unit -- user_scope_swap`
Expected: pass.

- [ ] **Step 5: Add a rollback test for swap-failure**

Append:

```rust
#[test]
fn user_scope_swap_rolls_back_when_new_namespace_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().to_path_buf();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    // Working namespace `foo`.
    {
        let ns_dir = registry.namespace_dir("foo");
        std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
        std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"foo body").unwrap();
        std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "foo"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#).unwrap();
    }
    // Broken namespace `bar` — references a missing adapter.
    {
        let ns_dir = registry.namespace_dir("bar");
        std::fs::create_dir_all(&ns_dir).unwrap();
        std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "bar"
[adapters.does-not-exist]
user_files = [".claude/foo.md"]
"#).unwrap();
    }

    let foo = aenv_core::identity::NamespaceId::new("foo").unwrap();
    let bar = aenv_core::identity::NamespaceId::new("bar").unwrap();
    aenv_core::activate::swap_or_activate_user(&fs, &registry, &adapters, &fake_home, &foo).unwrap();
    let err = aenv_core::activate::swap_or_activate_user(&fs, &registry, &adapters, &fake_home, &bar).unwrap_err();
    // Some failure surfaced — but foo should still be active.
    let _ = err;
    let body = std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap();
    assert_eq!(body, b"foo body", "foo must be reactivated after bar failed");
}
```

Run + ensure it passes (it should — `swap_or_activate_user` reactivates on failure).

- [ ] **Step 6: Lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: swap_or_activate_user (transactional global activation swap)"
```

---

# Milestone D — Locking

## Task 11: `~/.aenv/global.lock`

**Files:**
- Create: `crates/aenv-core/src/global_lock.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Create: `crates/aenv-core/tests/global_lock.rs`
- Modify: `crates/aenv-core/src/error.rs` — add `GlobalConflict` variant

**Why:** Per the issue: "A `~/.aenv/global.lock` file prevents two concurrent `global use` / `global deactivate` invocations. Stale locks (>5min, no live pid) auto-cleared." This is a small file holding `{pid: u32, started_at: i64}`. Acquire writes; release deletes.

- [ ] **Step 1: Add new exit-code variant**

In `crates/aenv-core/src/error.rs`:

```rust
    /// Global activation conflict (concurrent global use, orphan stash,
    /// schema mismatch on global-state.json). Exit 19.
    #[error("global conflict: {0}")]
    GlobalConflict(String),
```

And in `exit_code()`:

```rust
            AenvError::GlobalConflict(_) => 19,
```

- [ ] **Step 2: Write the failing test**

Create `crates/aenv-core/tests/global_lock.rs`:

```rust
use aenv_core::global_lock::{acquire_global_lock, release_global_lock, LockHandle};
use aenv_core::AenvError;

#[test]
fn lock_acquire_and_release_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let lock_path = tmp.path().join("global.lock");
    let handle: LockHandle = acquire_global_lock(&lock_path).unwrap();
    assert!(lock_path.exists());
    release_global_lock(handle).unwrap();
    assert!(!lock_path.exists());
}

#[test]
fn lock_rejects_when_held_by_live_pid() {
    let tmp = tempfile::tempdir().unwrap();
    let lock_path = tmp.path().join("global.lock");
    let _h1 = acquire_global_lock(&lock_path).unwrap();
    let err = acquire_global_lock(&lock_path).unwrap_err();
    assert!(matches!(err, AenvError::GlobalConflict(_)));
}

#[test]
fn lock_clears_stale_lock_older_than_five_minutes() {
    let tmp = tempfile::tempdir().unwrap();
    let lock_path = tmp.path().join("global.lock");
    // Write a stale lock by hand: a pid that surely doesn't exist + old timestamp.
    let stale = serde_json::json!({"pid": 999_999_999u32, "started_at": 0i64});
    std::fs::write(&lock_path, serde_json::to_vec_pretty(&stale).unwrap()).unwrap();
    // acquire should succeed by clearing it.
    let h = acquire_global_lock(&lock_path).unwrap();
    release_global_lock(h).unwrap();
}
```

- [ ] **Step 3: Run + see it fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test global_lock`
Expected: module doesn't exist.

- [ ] **Step 4: Implement**

Create `crates/aenv-core/src/global_lock.rs`:

```rust
//! User-scope activation lock. Prevents two `aenv global …` invocations from
//! racing on the same `$HOME` / `$AENV_HOME`. Stale-lock detection (PID gone
//! or older than 5 minutes) auto-clears so a crashed-mid-flight previous run
//! does not permanently wedge global commands.

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Five-minute stale threshold, per the issue spec.
const STALE_SECS: i64 = 300;

#[derive(Debug, Serialize, Deserialize)]
struct LockFile {
    pid: u32,
    started_at: i64,
}

/// RAII-ish handle. Caller must explicitly `release_global_lock` (we don't
/// implement Drop because we want the call site to surface release errors).
#[derive(Debug)]
pub struct LockHandle {
    path: PathBuf,
}

/// Try to acquire the lock at `path`. Creates parent directory if needed.
pub fn acquire_global_lock(path: &Path) -> Result<LockHandle> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // If the lock exists, decide stale-or-live.
    if path.exists() {
        let body = std::fs::read(path)?;
        match serde_json::from_slice::<LockFile>(&body) {
            Ok(existing) => {
                let now = now_secs();
                let pid_alive = pid_alive(existing.pid);
                if pid_alive && (now - existing.started_at) < STALE_SECS {
                    return Err(AenvError::GlobalConflict(format!(
                        "another aenv global command is running (pid {}, started {}s ago)",
                        existing.pid,
                        now.saturating_sub(existing.started_at),
                    )));
                }
                // Stale — fall through to overwrite.
                let _ = std::fs::remove_file(path);
            }
            Err(_) => {
                // Corrupt lock — treat as stale.
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let lf = LockFile { pid: std::process::id(), started_at: now_secs() };
    std::fs::write(path, serde_json::to_vec_pretty(&lf).unwrap())?;
    Ok(LockHandle { path: path.to_path_buf() })
}

/// Release a previously acquired lock. Idempotent: a missing lock file is OK
/// (we may have lost a race to a stale-clear from another process).
pub fn release_global_lock(handle: LockHandle) -> Result<()> {
    let _ = std::fs::remove_file(&handle.path);
    Ok(())
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // kill(pid, 0) returns 0 if the process is alive, -1 with ESRCH if not.
    // Safety: kill with signal 0 has no side effects.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    // Conservative: assume alive on non-Unix; stale-by-age covers the typical case.
    true
}
```

Add `libc = "0.2"` to `crates/aenv-core/Cargo.toml` (verify the workspace doesn't already pin it — if it does, use `libc.workspace = true`).

In `crates/aenv-core/src/lib.rs`: `pub mod global_lock;`.

- [ ] **Step 5: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test global_lock`
Expected: all pass.

- [ ] **Step 6: Lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: global.lock acquire/release with stale-PID + 5min auto-clear (exit 19)"
```

---

## Task 12: Wire the lock into `swap_or_activate_user` + `deactivate_namespace_in_scope`

**Files:**
- Modify: `crates/aenv-core/src/activate/mod.rs`
- Modify: `crates/aenv-core/src/deactivate.rs`
- Modify: `crates/aenv-core/tests/activate_global_unit.rs`

**Why:** All write-path globals must hold the lock for the duration.

- [ ] **Step 1: Wrap `swap_or_activate_user` to hold the lock**

In `swap_or_activate_user`, the first non-validation step is:

```rust
    let handle = crate::global_lock::acquire_global_lock(&layout.global_lock_path())?;
```

…and at every return point (Ok and Err), release the lock. Use a small scope-guard helper or `match` on the inner result and release explicitly.

- [ ] **Step 2: Same for `deactivate_namespace_in_scope` when scope = User**

Inside the function, after determining scope is User, acquire the lock. Don't acquire it for project scope — the project case has implicit per-directory exclusivity.

- [ ] **Step 3: Test that concurrent activation rejects with exit 19**

Append to `crates/aenv-core/tests/activate_global_unit.rs`:

```rust
#[test]
fn concurrent_global_activation_fails_with_global_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().to_path_buf();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(adapters_dir.join("claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"x").unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#).unwrap();

    // Simulate a held lock by acquiring one ourselves.
    let lock_path = registry.global_lock_path();
    let _h = aenv_core::global_lock::acquire_global_lock(&lock_path).unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let err = aenv_core::activate::swap_or_activate_user(
        &fs, &registry, &adapters, &fake_home, &leaf,
    ).unwrap_err();
    assert_eq!(err.exit_code(), 19);
    assert!(matches!(err, aenv_core::AenvError::GlobalConflict(_)));
}
```

- [ ] **Step 4: Run + lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test activate_global_unit
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: global lock guards swap_or_activate_user + deactivate_user"
```

---

# Milestone E — CLI surface

## Task 13: `aenv global` subcommand tree skeleton

**Files:**
- Modify: `crates/aenv-cli/src/main.rs`
- Create: `crates/aenv-cli/src/cmd/global/mod.rs`
- Create: `crates/aenv-cli/src/cmd/global/use_.rs`
- Create: `crates/aenv-cli/src/cmd/global/deactivate.rs`
- Create: `crates/aenv-cli/src/cmd/global/status.rs`
- Create: `crates/aenv-cli/src/cmd/global/which.rs`
- Create: `crates/aenv-cli/src/cmd/global/list.rs`
- Create: `crates/aenv-cli/src/cmd/global/doctor.rs`
- Create: `crates/aenv-cli/src/cmd/global/diff.rs`
- Modify: `crates/aenv-cli/src/cmd/mod.rs`
- Modify: `crates/aenv-cli/src/lib.rs`

**Why:** Wire up `clap`'s tree before any subcommand has real behavior — gives us a stable surface to test against in subsequent tasks.

- [ ] **Step 1: Add clap derives**

In `crates/aenv-cli/src/main.rs`, add to the top-level `Command` enum:

```rust
    /// User-global activation surface (`~/.claude/`, `~/.codex/`, …). Mirrors
    /// the project-local verbs but operates on `$HOME` instead of the project.
    Global {
        #[command(subcommand)]
        action: GlobalAction,
    },
```

And below the existing `enum CacheAction { … }`:

```rust
#[derive(Debug, Subcommand)]
enum GlobalAction {
    /// Activate a namespace's user-scope files into `$HOME`. Replaces any
    /// existing global activation in a single transaction.
    Use { name: String },
    /// Reverse `aenv global use`: restore stashed originals, delete state.
    Deactivate,
    /// Show the active global namespace and managed files.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Show which global namespace manages a given user-scope path.
    Which {
        /// User-scope path (e.g. `~/.claude/CLAUDE.md` or `.claude/CLAUDE.md`).
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// List namespaces that declare user-scope files.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Evaluate policies against a namespace's user-scope candidates.
    Doctor {
        namespace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Diff user-scope content against the active global activation or between
    /// two namespaces' user-scope subsets.
    Diff {
        ns_a: Option<String>,
        ns_b: Option<String>,
        #[arg(long)]
        json: bool,
    },
}
```

- [ ] **Step 2: Stub the dispatch arm**

In `main`, add to the `match cli.command { … }`:

```rust
            Command::Global { action } => {
                let fake_home = std::env::var("HOME")
                    .map(std::path::PathBuf::from)
                    .map_err(|_| aenv_core::AenvError::ManifestInvalid(
                        "HOME not set; aenv global requires HOME".into()))?;
                match action {
                    GlobalAction::Use { name } => {
                        let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                            &fs, &layout.adapters_dir(),
                        )?;
                        cmd::global::use_::run(&fs, &layout, &adapters, &fake_home, &name)
                    }
                    GlobalAction::Deactivate => {
                        cmd::global::deactivate::run(&fs, &layout, &fake_home)
                    }
                    GlobalAction::Status { json } => {
                        cmd::global::status::run(&fs, &layout, &fake_home, json)
                    }
                    GlobalAction::Which { path, json } => {
                        cmd::global::which::run(&fs, &layout, &fake_home, &path, json)
                    }
                    GlobalAction::List { json } => {
                        cmd::global::list::run(&fs, &layout, json)
                    }
                    GlobalAction::Doctor { namespace, json } => {
                        let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                            &fs, &layout.adapters_dir(),
                        )?;
                        cmd::global::doctor::run(&fs, &layout, &adapters, namespace.as_deref(), json)
                    }
                    GlobalAction::Diff { ns_a, ns_b, json } => {
                        cmd::global::diff::run(&fs, &layout, &fake_home, ns_a.as_deref(), ns_b.as_deref(), json)
                    }
                }
            }
```

- [ ] **Step 3: Create stub modules**

Each of `crates/aenv-cli/src/cmd/global/{use_,deactivate,status,which,list,doctor,diff}.rs` gets a `run` function that returns `Ok(())` with `todo!()` or a minimal "not implemented" stub. Tasks 14–18 fill them in.

`crates/aenv-cli/src/cmd/global/mod.rs`:

```rust
pub mod deactivate;
pub mod diff;
pub mod doctor;
pub mod list;
pub mod status;
pub mod use_;
pub mod which;
```

`crates/aenv-cli/src/cmd/mod.rs` — add `pub mod global;` to the existing list.

- [ ] **Step 4: Verify the binary parses**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --workspace`
Expected: builds.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo run -p aenv-cli -- global --help`
Expected: subcommand list shows `use`, `deactivate`, `status`, `which`, `list`, `doctor`, `diff`.

- [ ] **Step 5: Lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: aenv global subcommand tree skeleton (stubs only)"
```

---

## Task 14: `aenv global use <ns>` end-to-end

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/use_.rs`
- Create: `crates/aenv-cli/tests/global_use_e2e.rs`

**Why:** Wire the CLI to `swap_or_activate_user`. The e2e test uses `--project` and an `AENV_HOME` override to redirect activation onto a tempdir, so the test doesn't touch the developer's real `~/.claude`.

- [ ] **Step 1: Write the failing e2e test**

Create `crates/aenv-cli/tests/global_use_e2e.rs`:

```rust
use assert_cmd::Command;
use std::path::PathBuf;

fn aenv() -> Command {
    Command::cargo_bin("aenv").unwrap()
}

#[test]
fn global_use_activates_user_files_under_home_override() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(aenv_home.join("adapters/claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let ns_dir = aenv_home.join("envs/ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new").unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#).unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "use", "ns"])
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let materialized = fake_home.join(".claude/CLAUDE.md");
    assert!(materialized.exists());
    assert_eq!(std::fs::read(&materialized).unwrap(), b"new");
    assert!(aenv_home.join("global-state.json").exists());
}
```

- [ ] **Step 2: Run it to fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test global_use_e2e`
Expected: failure (stub returns `Ok(())` but doesn't activate).

- [ ] **Step 3: Implement `cmd::global::use_::run`**

`crates/aenv-cli/src/cmd/global/use_.rs`:

```rust
use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    fake_home: &Path,
    name: &str,
) -> Result<()> {
    let leaf = NamespaceId::new(name)
        .map_err(|e| aenv_core::AenvError::ManifestInvalid(e.to_string()))?;
    let state = aenv_core::activate::swap_or_activate_user(fs, layout, adapters, fake_home, &leaf)?;
    println!(
        "Activated '{}' globally — {} file{} materialized under {}.",
        state.active_namespace,
        state.managed_files.len(),
        if state.managed_files.len() == 1 { "" } else { "s" },
        fake_home.display()
    );
    println!("Note: running harness sessions retain their previous config until restart.");
    Ok(())
}
```

- [ ] **Step 4: Run the test**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test global_use_e2e`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: aenv global use <ns> activates user-scope files"
```

---

## Task 15: `aenv global deactivate`

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/deactivate.rs`
- Modify: `crates/aenv-cli/tests/global_use_e2e.rs` (extend with deactivate)

**Why:** Symmetric companion to Task 14.

- [ ] **Step 1: Extend the e2e test**

Append to `crates/aenv-cli/tests/global_use_e2e.rs`:

```rust
#[test]
fn global_deactivate_restores_stash() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original").unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(aenv_home.join("adapters/claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let ns_dir = aenv_home.join("envs/ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new").unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#).unwrap();

    aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "use", "ns"])
        .assert()
        .success();
    aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "deactivate"])
        .assert()
        .success();
    assert_eq!(std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(), b"original");
    assert!(!aenv_home.join("global-state.json").exists());
}

#[test]
fn global_deactivate_with_nothing_active_is_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("no global activation"), "got: {stdout}");
}
```

- [ ] **Step 2: Implement `cmd::global::deactivate::run`**

```rust
use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    fake_home: &Path,
) -> Result<()> {
    if !fs.exists(&layout.global_state_path())? {
        println!("no global activation to deactivate");
        return Ok(());
    }
    let active = aenv_core::deactivate::deactivate_namespace_in_scope(
        fs, layout, fake_home, aenv_core::scope::Scope::User,
    )?;
    println!("Deactivated '{active}' globally.");
    Ok(())
}
```

- [ ] **Step 3: Run + lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test global_use_e2e
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: aenv global deactivate restores stash"
```

---

## Task 16: `aenv global status` (+ JSON shape)

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/status.rs`
- Modify: `crates/aenv-core/src/json/` (add `global_status` module)
- Create: `crates/aenv-cli/tests/global_status_e2e.rs`
- Create: insta snapshot under `crates/aenv-core/tests/snapshots/`

**Why:** Matches the existing `aenv status` shape so scripts can poll global state symmetrically.

- [ ] **Step 1: Sketch the JSON shape**

Same as `aenv status --json` plus a `"scope": "user"` top-level field. Reuse the existing `json::status` builder if it's flexible enough; otherwise create a sibling.

- [ ] **Step 2: Write the failing test**

Create `crates/aenv-cli/tests/global_status_e2e.rs`:

```rust
use assert_cmd::Command;

fn aenv() -> Command {
    Command::cargo_bin("aenv").unwrap()
}

#[test]
fn global_status_without_activation_is_empty_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let out = aenv()
        .env("AENV_HOME", tmp.path().join(".aenv"))
        .env("HOME", tmp.path().join("home"))
        .args(["global", "status"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("no global activation") || s.contains("inactive"));
}

#[test]
fn global_status_json_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(aenv_home.join("adapters/claude-code.toml"), r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#).unwrap();
    let ns_dir = aenv_home.join("envs/ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"x").unwrap();
    std::fs::write(ns_dir.join("aenv.toml"), r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#).unwrap();
    aenv().env("AENV_HOME", &aenv_home).env("HOME", &fake_home)
        .args(["global", "use", "ns"]).assert().success();
    let out = aenv().env("AENV_HOME", &aenv_home).env("HOME", &fake_home)
        .args(["global", "status", "--json"])
        .output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["scope"], "user");
    assert_eq!(json["active_namespace"], "ns");
    assert!(json["managed_files"].is_array());
}
```

- [ ] **Step 3: Implement `cmd::global::status::run`**

```rust
use aenv_core::error::Result;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    fake_home: &Path,
    json: bool,
) -> Result<()> {
    let state_path = layout.global_state_path();
    if !fs.exists(&state_path)? {
        if json {
            println!("{}", serde_json::json!({"scope": "user", "active": false}));
        } else {
            println!("no global activation");
        }
        return Ok(());
    }
    let bytes = fs.read(&state_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| aenv_core::AenvError::ManifestInvalid(e.to_string()))?;
    let state = aenv_core::state::ActivationState::from_json(text)?;
    if json {
        let body = serde_json::to_string_pretty(&serde_json::json!({
            "scope": "user",
            "active": true,
            "active_namespace": state.active_namespace,
            "target_root": fake_home,
            "managed_files": state.managed_files.iter().map(|m| serde_json::json!({
                "path": m.path,
                "strategy": m.strategy,
            })).collect::<Vec<_>>(),
            "backed_up": state.backed_up.iter().map(|b| serde_json::json!({
                "original_path": b.original_path,
                "backup_path": b.backup_path,
            })).collect::<Vec<_>>(),
        })).unwrap();
        println!("{body}");
    } else {
        println!("Active global namespace: {}", state.active_namespace);
        println!("Target root: {}", fake_home.display());
        println!("Managed files: {}", state.managed_files.len());
        for m in &state.managed_files {
            println!("  ~/{}", m.path.display());
        }
        println!("Note: running harness sessions retain their previous config until restart.");
    }
    Ok(())
}
```

- [ ] **Step 4: Run + lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test global_status_e2e
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: aenv global status (text + JSON, scope=user)"
```

---

## Task 17: `aenv global which`, `list`, `diff`

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/{which,list,diff}.rs`
- Create: `crates/aenv-cli/tests/global_readonly_e2e.rs`

**Why:** Read-only verbs. They mirror the project shapes — `aenv which`, `aenv list`, `aenv diff` — but filter to user scope. Mostly composition of existing core functions.

- [ ] **Step 1: Write a single e2e suite that exercises all three**

```rust
use assert_cmd::Command;

fn aenv() -> Command { Command::cargo_bin("aenv").unwrap() }

#[test]
fn global_which_returns_managing_namespace() {
    // … set up a global activation, then run `aenv global which ~/.claude/CLAUDE.md`
    // and assert stdout names the active namespace.
}

#[test]
fn global_list_includes_only_namespaces_with_user_files() {
    // Create three namespaces: one with user_files, one without, one with both.
    // Verify `aenv global list` only shows the two with user_files.
}

#[test]
fn global_diff_drift_reports_changed_bytes() {
    // Activate a namespace user-scope, edit ~/.claude/CLAUDE.md in place, then
    // run `aenv global diff` and assert it reports drift on that file.
}
```

(Each test body follows the existing `tests/global_use_e2e.rs` pattern. Fill in concretely while implementing — keep paths absolute, use `AENV_HOME` + `HOME` overrides.)

- [ ] **Step 2: Implement each `run` function**

For `which`: load global state, find the managed file whose path matches the queried path (normalize `~/` prefix to `<fake_home>/` to compare), print the qualified name. JSON mirror: `{"scope":"user","path":…,"qualified":…,"shadows":[…]}`.

For `list`: walk every namespace in the registry; for each, parse manifest and check if any `[adapters.*].user_files` array is non-empty OR if any `[[skills]]` has `scope = "user"`. Print the matching names.

For `diff`: project-side diff splits into "drift" (active state vs. on-disk) and "structural" (two namespaces). User-scope diff is identical in shape, target root = `$HOME`.

- [ ] **Step 3: Run + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test global_readonly_e2e
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: aenv global which | list | diff (read-only verbs)"
```

---

## Task 18: `aenv use <ns> --global` sugar

**Files:**
- Modify: `crates/aenv-cli/src/main.rs` (Use command — add `--global` flag)
- Modify: `crates/aenv-cli/src/cmd/use_.rs`
- Modify: `crates/aenv-cli/tests/global_use_e2e.rs`

**Why:** Per the issue: "`aenv use <ns> --global` (sugar) does both" project + global activation in one call.

- [ ] **Step 1: Add the flag**

In `main.rs` `Command::Use { … }`:

```rust
    Use {
        name: String,
        #[arg(long)] project: Option<PathBuf>,
        /// Also activate this namespace's user-scope files (sugar for
        /// `aenv use foo && aenv global use foo`).
        #[arg(long)]
        global: bool,
    },
```

- [ ] **Step 2: Update the dispatch**

```rust
            Command::Use { name, project, global } => {
                let project_root = paths::resolve_project_root_for_pin(&fs, project)?;
                cmd::use_::run(&fs, &layout, &project_root, &name)?;
                if global {
                    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                        &fs, &layout.adapters_dir(),
                    )?;
                    let fake_home = std::env::var("HOME")
                        .map(std::path::PathBuf::from)
                        .map_err(|_| aenv_core::AenvError::ManifestInvalid(
                            "HOME not set".into()))?;
                    cmd::global::use_::run(&fs, &layout, &adapters, &fake_home, &name)?;
                }
                Ok(())
            }
```

- [ ] **Step 3: Add a test that exercises the sugar**

```rust
#[test]
fn use_with_global_flag_activates_both_scopes() {
    // Activate `ns` with `aenv use ns --global` and verify both
    // <project>/CLAUDE.md (project) and <fake_home>/.claude/CLAUDE.md (user)
    // are materialized in a single command.
}
```

- [ ] **Step 4: Run + lint + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test global_use_e2e
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: aenv use <ns> --global sugar (project + global in one command)"
```

---

# Milestone F — Doctor + drift + orphan stash

## Task 19: `aenv global doctor` end-to-end

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/doctor.rs`
- Create: `crates/aenv-cli/tests/global_doctor_e2e.rs`

**Why:** Surface user-scope policy violations (instructions over soft-limit, forbidden paths) the same way `aenv doctor` does for project scope, with target labels showing `~/`-prefixed paths so users see what they'll see post-activation.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn global_doctor_reports_user_scope_oversize_instructions() {
    // Build a namespace whose user-scope CLAUDE.md is 6000 chars while the
    // claude-code adapter declares a 5000 user_soft_limits.instructions.
    // Run `aenv global doctor ns` and assert stderr/stdout names the
    // ~/.claude/CLAUDE.md path and the limit.
}
```

- [ ] **Step 2: Implement**

`cmd::global::doctor::run` resolves the namespace, calls `doctor::evaluate`, filters outcomes to those whose target is a user-scope candidate (we already track that in Task 6), prints them with `~/`-prefixed labels.

- [ ] **Step 3: Run + commit**

---

## Task 20: Orphan-stash detection (`aenv global doctor` + `aenv global deactivate --prune`)

**Files:**
- Modify: `crates/aenv-cli/src/cmd/global/doctor.rs`
- Modify: `crates/aenv-cli/src/cmd/global/deactivate.rs` (add `--prune` flag)
- Modify: `crates/aenv-core/src/global_lock.rs` or new module
- Create: `crates/aenv-cli/tests/global_orphan_stash_e2e.rs`

**Why:** Per the issue's exit-code 19 spec: "stash dir non-empty with no recorded state → error 19 with hint 'orphan stash detected; run `aenv global doctor --recover` or remove the directory manually.'" Plus a deactivate `--prune` that removes orphan timestamped stashes once activation is gone.

- [ ] **Step 1: Test**

```rust
#[test]
fn global_doctor_reports_orphan_stash() {
    // Manually create $AENV_HOME/global-stash/<timestamp>/ with a file in it,
    // no global-state.json. Run `aenv global doctor` and expect a non-zero
    // exit with mention of "orphan stash" in stderr.
}

#[test]
fn global_deactivate_prune_removes_orphan_stash() {
    // Same setup. Run `aenv global deactivate --prune`; expect the stash dir
    // to be gone after.
}
```

- [ ] **Step 2: Implement orphan detection**

A small helper in `crates/aenv-core/src/state.rs` (or a new `crates/aenv-core/src/global.rs`):

```rust
/// Detect orphan stashes: timestamped subdirs of `<aenv_home>/global-stash/`
/// that have no corresponding entry in `<aenv_home>/global-state.json`.
pub fn list_orphan_stashes(layout: &RegistryLayout) -> Result<Vec<PathBuf>> { … }
```

CLI: `doctor` reports them; `deactivate --prune` calls `remove_dir_all` on each.

- [ ] **Step 3: Run + commit**

---

# Milestone G — End-to-end + builtin coverage

## Task 21: Update builtin adapter TOMLs with `user_files`

**Files:**
- Modify: `crates/aenv-core/src/adapters_builtin/claude_code.toml`
- Modify: `crates/aenv-core/src/adapters_builtin/codex.toml`
- Modify: `crates/aenv-core/src/adapters_builtin/cursor.toml`
- Modify: `crates/aenv-core/tests/adapters_builtin.rs`

**Why:** Once schema is stable, populate the three adapters that have meaningful user-level surfaces. The other four (`aider`, `cline`, `continue_`, `windsurf`, `mcp`) leave `user_files` empty unless their docs say otherwise (research as part of this task).

- [ ] **Step 1: Research user-level layouts**

Run: `grep -rn "user-level\|~/.cursor\|~/.codex\|~/.claude" pm_docs/ | head -40`

Codex: `~/.codex/AGENTS.md`, `~/.codex/config.toml` (per existing memory + the issue).
Claude Code: `~/.claude/CLAUDE.md`, `~/.claude/agents/`, `~/.claude/commands/`, `~/.claude/hooks/`, `~/.claude/settings.json`, `~/.claude/skills/`.
Cursor: `~/.cursor/rules/` per Cursor docs (verify against `pm_docs/aenv_adapter_research.md`).

- [ ] **Step 2: Update each adapter file**

`claude_code.toml`:

```toml
name = "claude-code"
files = ["CLAUDE.md", ".claude/"]
skills_dir = ".claude/skills"

user_files = ["~/.claude/CLAUDE.md", "~/.claude/agents/", "~/.claude/commands/",
              "~/.claude/hooks/", "~/.claude/settings.json"]
user_skills_dir = "~/.claude/skills"

[roles]
"CLAUDE.md" = "instructions"

[user_roles]
"~/.claude/CLAUDE.md" = "instructions"

[soft_limits]
instructions = 5000

[user_soft_limits]
instructions = 5000

[user_default_merge]
"~/.claude/settings.json" = "deep"

[[parameters]]
name = "instructions_budget"
type = "integer"
```

…and similarly for codex + cursor.

- [ ] **Step 3: Update the `adapters_builtin.rs` test**

Snapshot the parsed shape with `insta` so future schema changes that break the embedded TOMLs are caught.

- [ ] **Step 4: Run + commit**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test adapters_builtin
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Issue #4: builtin adapters declare user_files (claude-code, codex, cursor)"
```

---

## Task 22: Live `$HOME`-redirected integration test

**Files:**
- Create: `crates/aenv-cli/tests/global_full_cycle_e2e.rs`

**Why:** Per the issue's test plan: "Live integration on a sacrificial `$HOME` (tmpdir as HOME env override): full activate → swap → deactivate cycle against a real `~/.claude/` layout with agents + commands + settings.json."

- [ ] **Step 1: Write the integration test**

```rust
//! Full-cycle integration: an aenv user activates ns1 globally, swaps to ns2,
//! deactivates, and recovers from an orphan stash — all against a tempdir
//! standing in for the user's $HOME. Touches every public verb at least once.

#[test]
fn live_cycle_activate_swap_deactivate() {
    // (~300 lines — set up two namespaces with multi-file user-scope content
    // covering symlink, section-merge, and deep-merge strategies.)
}
```

(The test body is concrete; write it in full when implementing — no placeholder code in the final plan execution.)

- [ ] **Step 2: Run + commit**

---

## Task 23: Cross-machine hash stability for user-scope subsets

**Files:**
- Modify: `crates/aenv-core/src/hash.rs` (add `hash_resolved_namespace_user`)
- Modify: `crates/aenv-core/tests/cross_machine_hash.rs`

**Why:** R-84 (hash) is public contract. User-scope hash needs the same properties: stable across machines, shadow-blind, manifest-formatting-blind. Add `hash_resolved_namespace_user` that filters candidates to `Scope::User` then runs the same canonicalization pipeline; project-side `hash_resolved_namespace` filters to `Scope::Project` (which is a no-op for pre-existing namespaces — their hashes don't change).

- [ ] **Step 1: Extend the cross-machine fixture**

The existing fixture only covers project-scope content. Add a user-scope subset and lock its expected hash.

- [ ] **Step 2: Update hash.rs**

Two functions, both calling into a shared `hash_candidates(scope, …)` core.

- [ ] **Step 3: Verify**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --test cross_machine_hash --test hash_basic --test hash_properties
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "Issue #4: hash_resolved_namespace_user (per-scope hashing, R-84 extended)"
```

---

# Milestone H — Docs + walkthrough + release

## Task 24: README + walkthrough

**Files:**
- Modify: `README.md` (add "Global namespaces" section)
- Create: `pm_docs/walkthrough-global-namespaces.md`

**Why:** Per the issue's test-plan tail: "README + walkthroughs gain a 'Global namespaces' section showing the research-mode example end-to-end."

- [ ] **Step 1: README section**

A new H2 after the existing "Switching namespaces" section. Show:
- The motivating "research mode" example.
- `aenv global use research`
- `aenv global status`
- `aenv global deactivate`
- The running-session caveat verbatim.
- A pointer to the walkthrough.

- [ ] **Step 2: Walkthrough**

Concrete step-by-step with code blocks, modeled on `pm_docs/walkthrough-three-harnesses.md`. Cover:
1. Create `research` namespace with one user-scope agent and a custom `~/.claude/settings.json` snippet.
2. `aenv global use research` — verify `~/.claude/` content.
3. Create `default` namespace, swap.
4. Deactivate, observe the restored original `~/.claude/`.
5. Show `aenv global doctor` catching an oversize `~/.claude/CLAUDE.md`.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "Docs: README + walkthrough for global namespaces"
```

---

## Task 25: CHANGELOG + version bump + tag

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml` (workspace.package.version)

**Why:** Standard release hygiene. The repo's RELEASING.md will tell you exactly how the previous releases were tagged; follow it.

- [ ] **Step 1: Read `RELEASING.md`**

Confirm the version-bump policy. Issue #4 is a new feature; per semver while pre-1.0, this is a minor bump: `0.0.3` → `0.1.0` (or `0.0.4` if the repo is reserving 0.1 for a bigger milestone — read RELEASING.md before deciding).

- [ ] **Step 2: CHANGELOG entry**

```markdown
## 0.1.0 — YYYY-MM-DD

### Added
- `aenv global` subcommand tree: activate user-scope files (`~/.claude/`, `~/.codex/`)
  across a single namespace switch. Symmetric verbs: `use`, `deactivate`, `status`,
  `which`, `list`, `doctor`, `diff`. Sugar form: `aenv use <ns> --global`.
- Adapter TOML schema: `user_files`, `user_roles`, `user_soft_limits`,
  `user_default_merge`, `user_skills_dir`, `user_merge_strategies`.
- Namespace manifest: `[adapters.<name>] user_files` + per-file `user_merge`.
- `[[skills]] scope = "user" | "project"` (default: project).
- New state file at `$AENV_HOME/global-state.json` and stash at
  `$AENV_HOME/global-stash/<ts>/`.
- Lock at `$AENV_HOME/global.lock` (stale-PID + 5min auto-clear).
- Exit code 19 — `GlobalConflict`.
- `hash_resolved_namespace_user` — R-84 extended to user scope.

### Changed
- `activate_namespace` is now a wrapper for `activate_namespace_in_scope(…, Scope::Project, …)`.
  Public signature unchanged; semantics identical for existing project usage.
- `Candidate` gains a `scope` field; pre-existing candidates default to `Scope::Project`.
- `ActivationState` gains `scope`; old state files default to `Project` on read.
- `SCHEMA_VERSION` bumped from 4 to 5.

### Documentation
- README "Global namespaces" section.
- `pm_docs/walkthrough-global-namespaces.md`.
```

- [ ] **Step 3: Bump version + commit + tag**

```bash
# After CHANGELOG + version bump:
git add Cargo.toml CHANGELOG.md
git commit -m "Release: vX.Y.Z — global namespaces (Issue #4)"
git tag -a vX.Y.Z -m "global namespaces"
```

(Do NOT push the tag without the user's explicit go-ahead — the project's RELEASING.md will spell that out.)

- [ ] **Step 4: Final lint + full test sweep**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --all --check
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace
```

Expected: everything green; total test count ≈ existing 448 + ~30 new.

- [ ] **Step 5: Close the issue**

```bash
gh issue close 4 --comment "Shipped in vX.Y.Z. See CHANGELOG and pm_docs/walkthrough-global-namespaces.md."
```

---

# Self-review checklist (run before kicking off execution)

- [ ] Every issue-spec section maps to at least one task: CLI surface (Tasks 13–18), Adapter schema (Task 2), Namespace manifest scope (Tasks 3 + 4), Activation flow (Tasks 7–10), Deactivation (Task 9), Locking (Tasks 11–12), Exit code 19 (Task 11), State file with `scope` field (Task 7), Edge cases — atomicity probe (Task 7), orphan stash (Task 20), stash conflict (Task 20), symlink semantics (Task 8), running-session caveat (Tasks 14 + 16 print it), soft-limit violations (Task 6), extends mixing scopes (Task 5 — both gathered, filtered by activate). Doctor drift (Task 20). Tests (every milestone).
- [ ] No placeholders in tasks already at full detail (1–16). Tasks 17, 19, 20, 22 contain stub test bodies — the implementing agent should expand them to concrete code following the patterns in tasks 14–16 before checking off the task.
- [ ] Type consistency check: `Scope` is the same enum throughout; `Candidate.scope` introduced in Task 5 is read by Task 6 (doctor) and Task 7 (activate); `ActivationState.scope` introduced in Task 7 is read by Task 9 (deactivate) and Task 16 (status). `RegistryLayout::{global_state_path, global_stash_root, global_lock_path}` introduced in Task 7 are used by Tasks 9, 11, 14, 15, 16, 20.
- [ ] Hash R-84 contract: Task 23 adds `hash_resolved_namespace_user` and verifies existing project hashes are unchanged. Pre-existing namespaces with no `user_files` still hash identically pre- and post-feature.
- [ ] Exit codes 10–20 stable: only one new code added (19), per the issue's allocation.

---

# Execution

Once this plan is approved, kick off via `superpowers:subagent-driven-development`:
- Dispatch fresh subagent per task (or per pair of tightly-coupled tasks).
- Review between tasks: lint clean, tests green, commit landed.
- Reject anything that adds error handling for "can't happen" cases — see the workspace CLAUDE.md.
