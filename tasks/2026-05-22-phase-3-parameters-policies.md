# Phase 3 — Parameters & Policies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Manifests declare typed `[parameters]` (string / int / bool / list-of-string) and `[policies]` (advisory or `enforce = true`). Parameters inherit through the `extends` chain with last-wins semantics; their effective values are recorded in activation state. Policies inherit with the rule that children can add or tighten, never silently weaken a parent's `enforce = true`. `aenv get <ns>.<param>` and `aenv get .<param>` (active project) print the effective value plus the qualified namespace that supplied it. `aenv set <ns>.<param> <value>` updates a namespace's manifest. `aenv doctor [--ns <name>]` evaluates the union of own + inherited policies against the namespace's artifacts and prints per-policy outcomes; `enforce = true` violations also block `aenv activate` with exit 17.

**Architecture:** Parameters and policies are pure data layered on top of Phase 2's resolution result. The `resolve_namespace` function gains two siblings — `resolve_parameters` and `resolve_policies` — that walk the same chain and collapse `[parameters]`/`[policies]` tables into `BTreeMap<String, Resolved*>`. The `ResolutionResult` type grows two fields; `ActivationState` grows two fields and bumps to schema 3 (schema 2 still reads, with empty maps as defaults). The four built-in policy evaluators live in a `policies/builtin/` module behind a `PolicyEvaluator` trait. `aenv doctor` walks `ResolvedNamespace` + filesystem and returns a `DoctorReport` whose serialization is a Phase 5 problem; the CLI prints text only. The enforce-block at activation reuses the same evaluator, short-circuiting before any file is written so the project remains untouched (R-63).

**Tech Stack:** Rust 1.85+ stable. No new external crates (we already have `serde_yaml`, `toml`, `serde_json`, `thiserror`). New library deps: none. New CLI deps: none.

**Plan structure:** 20 tasks. T1–T2 build pure parameter primitives (type, manifest parse). T3 resolves parameter inheritance. T4–T5 build the adapter-side declaration + R-71 type-incompat check. T6–T7 are policy primitives + inheritance with enforce-protection. T8 sets up the `PolicyEvaluator` scaffold. T9–T12 implement the four built-in policy evaluators (one per task; same pattern as Phase 2's merge primitives). T13 threads parameters + policies through `ResolvedNamespace`, `ActivationState`, and the schema-3 bump. T14 builds the `DoctorReport` orchestrator. T15 wires the enforce-block into `activate_namespace`. T16–T17 add the `get` and `set` CLI subcommands. T18 adds `aenv doctor` and upgrades `aenv status`. T19 is the end-to-end CLI test covering spec §5.5 (parameter queries) and §5.12 (doctor clean + violation). T20 tags `phase-3-complete`. Estimated effort: 3–4 days of focused work, comparable to Phase 2.

**Repository state at start:** `main` at `393eb62` with `phase-2-complete` tagged. Working tree clean. All Phase 2 e2e tests pass. `error.rs` already declares `ParameterUndefined` (exit 16) and `PolicyViolation` (exit 17) — both are unused after Phase 2; this phase makes them live.

**Important Phase 0/1/2 invariants this plan honors:**

- `Filesystem` trait still uses `&self`. No new trait methods.
- `Filesystem::write(path, contents)` creates missing parent dirs by contract.
- `Filesystem::exists` returns `io::Result<bool>`; treat `Err` as "couldn't stat", not "not found".
- All paths below the CLI layer are absolute. The library never reads `std::env::current_dir()` or `std::env::var(...)`.
- State directory is `.aenv-state/` (not `.aenv/`).
- `AenvError` variants `ParameterUndefined` (exit 16) and `PolicyViolation` (exit 17) — already declared, made live in this phase. No new variants required.
- The materialized-path invariant continues to hold: no path on disk contains `::`. Parameters and policies live in state.json only.
- Tests anticipate rustfmt `max_width = 100`.
- Backup atomicity continues to cover every materialization path. Enforce-violation aborts BEFORE any backup or symlink happens.
- The `.` parameter-access separator and the `::` qualified-name separator are both public CLI/JSON contracts (engineering §7.5). Do not conflate them.

**Phase 3 deliberately defers:**

- Adapter parameter *projection* into tool-specific config files (R-68's second half: how `auto_invoke_subagents` lands in `.claude/settings.json`) — Phase 4 problem, once skills/agents exist. Phase 3 only parses the projection declaration in the adapter TOML; the projection mapping is recorded but not yet executed.
- `aenv doctor --json` and `aenv get --json` — Phase 5.
- Adapter-specific policy keys beyond the four built-ins — Phase 4 (e.g. `windsurf_max_chars`) or later.
- Soft size limits as default policies materialized when no manifest declares them — Phase 4 ships defaults; Phase 3 evaluates only what the manifests (after inheritance) actually declare.
- Skill-shaped policies (`skill_requires_description`) on imported-but-not-yet-resolved skills — Phase 4 problem. Phase 3 only inspects authored skills (files that already live in the namespace's tree) so the rule lands now without depending on the skill-import lifecycle.
- R-26's "effective soft limit shall be the lower of the adapter's documented limit and the namespace's declared `instructions_budget`" — Phase 3 supports authoring `instructions_max_chars` as a policy *and* `instructions_budget` as a parameter, but the *combining* of an adapter-shipped default with a namespace-declared budget waits for Phase 4 (which ships the adapter defaults). In Phase 3 the policy as authored is the effective limit; if a namespace wants a tighter budget it declares `instructions_max_chars` directly.

---

## File structure (created or modified in this phase)

**Library (`crates/aenv-core/src/`):**

| File | Responsibility |
|---|---|
| `parameters.rs` | `ParameterValue` enum (string / int / bool / list-of-string); TOML parse; `ResolvedParameter { value, source: NamespaceId }`; `resolve_parameters()` walks chain with last-wins |
| `policies.rs` | `PolicyDecl { value: PolicyValue, enforce: bool }`; `PolicyValue` enum (int / bool / list-of-string); TOML parse (shorthand + table form); `ResolvedPolicy`; `resolve_policies()` with R-75 enforcement |
| `policies/builtin/mod.rs` | `PolicyEvaluator` trait + dispatch by policy key |
| `policies/builtin/instructions_max_chars.rs` | Walks `role = "instructions"` artifacts; counts UTF-8 chars; emits a `PolicyOutcome` per file |
| `policies/builtin/skill_requires_description.rs` | Walks authored skills; parses YAML frontmatter; requires `description: ` to be present + non-empty |
| `policies/builtin/mcp_requires_command_or_url.rs` | Walks `.mcp.json`-shaped artifacts; requires each server entry to declare `command` or `url` |
| `policies/builtin/forbid_paths.rs` | List-of-glob; emits a `PolicyOutcome` per resolved artifact whose materialized path matches |
| `doctor.rs` | `DoctorReport`, `evaluate(resolved, fs, layout)`; the orchestrator |

**Library (modified):**

- `crates/aenv-core/src/lib.rs` — re-export `parameters`, `policies`, `doctor`
- `crates/aenv-core/src/manifest.rs` — `AenvManifest` gains `parameters: BTreeMap<String, ParameterValue>` and `policies: BTreeMap<String, PolicyDecl>`
- `crates/aenv-core/src/adapter.rs` — `Adapter` gains optional `parameters: Vec<AdapterParameterDecl>` (declaration only; projection deferred)
- `crates/aenv-core/src/resolve.rs` — `ResolutionResult` gains `parameters: BTreeMap<String, ResolvedParameter>`, `policies: BTreeMap<String, ResolvedPolicy>`
- `crates/aenv-core/src/state.rs` — `SCHEMA_VERSION` bumps to 3; `ActivationState` gains `parameters` and `policies` fields; schema-2 reader path defaults both to empty maps
- `crates/aenv-core/src/activate/mod.rs` — runs `enforce_policies_block` before any materialization; rejects with `PolicyViolation` (exit 17) if any `enforce = true` policy is violated
- `crates/aenv-core/src/adapters_builtin/mod.rs` — fix the `BUILTINS` array (currently only includes claude-code; should match `ALL`); no behavior change to existing tests, but the `install_builtins` symbol must seed all seven adapters so that `aenv create` followed by `aenv doctor` finds them

**Binary (`crates/aenv-cli/src/`):**

| File | Responsibility |
|---|---|
| `cmd/get.rs` | `aenv get [<ns>.]<param>` — prints effective value + qualified provenance |
| `cmd/set.rs` | `aenv set <ns>.<param> <value>` — rewrites the named namespace's `aenv.toml` `[parameters]` table |
| `cmd/doctor.rs` | `aenv doctor [--ns <name>]` — evaluates policies; exit 0 unless an `enforce = true` policy is violated, then exit 17 |
| `main.rs` (modify) | Add `Get`, `Set`, `Doctor` subcommands to clap |
| `cmd/mod.rs` (modify) | `pub mod get; pub mod set; pub mod doctor;` |
| `cmd/status.rs` (modify) | Append "Parameters" and "Policies" sections after the existing resolution chain output |

**Tests (new):**

- `crates/aenv-core/tests/parameters.rs` — type parsing, inheritance (single, chain, diamond), type-incompat across chain
- `crates/aenv-core/tests/policies.rs` — table-form parse, shorthand parse, inheritance, R-75 enforce-protection
- `crates/aenv-core/tests/policy_instructions_max_chars.rs` — clean + violation + boundary cases
- `crates/aenv-core/tests/policy_skill_requires_description.rs` — clean + violation + missing-frontmatter
- `crates/aenv-core/tests/policy_mcp_requires_command_or_url.rs` — clean + violation
- `crates/aenv-core/tests/policy_forbid_paths.rs` — exact match, glob match, no-match
- `crates/aenv-core/tests/doctor.rs` — full report assembly; precedence (own beats inherited only when not weakening)
- `crates/aenv-core/tests/state_schema.rs` — schema-2 read, schema-3 write/read round-trip, schema-3 with parameters/policies
- `crates/aenv-core/tests/activate_enforce.rs` — `enforce = true` violation blocks activation, leaves project untouched
- `crates/aenv-cli/tests/parameters_e2e.rs` — `create`, edit manifest, `get`, `set`, `get` again
- `crates/aenv-cli/tests/doctor_e2e.rs` — replicates functional spec §5.12 clean + violation cases

**Property tests (in `tests/parameters.rs` and `tests/policies.rs`):**

- Parameter inheritance is last-wins per-key: for any chain of length N and any key, the effective value equals the value declared in the latest namespace that declared it.
- A child manifest that declares the same key with the same TOML type as a parent never produces a type-incompat error (it always overrides cleanly).
- For any policy key with `enforce = true` in a parent, every chain in which the child's declaration would weaken it (downgrade enforce, raise an int limit, shorten a list deny-list) yields a `ManifestInvalid` from `resolve_policies`.

---

## Glossary (for the implementer)

- **ParameterValue** — a typed value declared in a namespace's `[parameters]` table. One of: string, integer, boolean, list-of-string. Other TOML types (table, datetime, float) are rejected at parse time with `ManifestInvalid`.
- **ResolvedParameter** — `{ value: ParameterValue, source: NamespaceId }`. The `source` is the latest namespace in the chain that declared this key.
- **AdapterParameterDecl** — a single entry in an adapter TOML's optional `[[parameters]]` array: declares a parameter name, expected type, and (deferred) projection target. Phase 3 uses these only for the type-compat check (R-71).
- **PolicyDecl** — a parsed `[policies]` entry: `{ value: PolicyValue, enforce: bool }`. Accepts both shorthand `key = <value>` (implies `enforce = false`) and table form `key = { value = ..., enforce = true }`.
- **PolicyValue** — int (e.g. `instructions_max_chars = 3000`), bool (e.g. `skill_requires_description = true`), or list-of-string (e.g. `forbid_paths = [".env*", "secrets/**"]`). Type is determined per known policy key; unknown keys parse but are flagged with a warning and skipped by the evaluator.
- **ResolvedPolicy** — `{ value: PolicyValue, enforce: bool, source: NamespaceId }`. The `source` is the *latest* namespace whose declaration of this key won the resolution; if a child added `enforce = true` to a key the parent declared advisory, the source is the child.
- **PolicyOutcome** — a single result emitted by an evaluator: `{ key, qualified_target, status: Pass | Warn { msg } | Fail { msg } }`. `Warn` for advisory; `Fail` for enforced-violation.
- **DoctorReport** — the report assembled by `evaluate()`. Contains the namespace chain, the resolved-policy table, and the flat list of `PolicyOutcome`s. The CLI renders this as text; Phase 5 will serialize as JSON.

---

### Task 1: `ParameterValue` type + TOML parsing

Pure types. No filesystem, no async. Owns the four allowed parameter shapes and rejects everything else.

**Files:**
- Create: `crates/aenv-core/src/parameters.rs`
- Modify: `crates/aenv-core/src/lib.rs` (add `pub mod parameters;`)
- Test: `crates/aenv-core/tests/parameter_value.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/parameter_value.rs`:

```rust
use aenv_core::parameters::ParameterValue;

#[test]
fn parses_string() {
    let pv: ParameterValue = ParameterValue::from_toml_value(&toml::Value::String(
        "claude-opus-4.7".to_owned(),
    ))
    .unwrap();
    assert_eq!(pv, ParameterValue::String("claude-opus-4.7".into()));
}

#[test]
fn parses_integer() {
    let pv = ParameterValue::from_toml_value(&toml::Value::Integer(3000)).unwrap();
    assert_eq!(pv, ParameterValue::Integer(3000));
}

#[test]
fn parses_boolean() {
    let pv = ParameterValue::from_toml_value(&toml::Value::Boolean(true)).unwrap();
    assert_eq!(pv, ParameterValue::Boolean(true));
}

#[test]
fn parses_list_of_strings() {
    let arr = toml::Value::Array(vec![
        toml::Value::String("code-reviewer".into()),
        toml::Value::String("write-tests".into()),
    ]);
    let pv = ParameterValue::from_toml_value(&arr).unwrap();
    assert_eq!(
        pv,
        ParameterValue::ListString(vec!["code-reviewer".into(), "write-tests".into()])
    );
}

#[test]
fn rejects_float() {
    let err = ParameterValue::from_toml_value(&toml::Value::Float(1.5)).unwrap_err();
    assert!(err.to_string().contains("float"));
}

#[test]
fn rejects_datetime() {
    let dt = toml::Value::Datetime("1979-05-27T07:32:00Z".parse().unwrap());
    let err = ParameterValue::from_toml_value(&dt).unwrap_err();
    assert!(err.to_string().contains("datetime"));
}

#[test]
fn rejects_inline_table() {
    let mut t = toml::value::Table::new();
    t.insert("k".into(), toml::Value::String("v".into()));
    let err = ParameterValue::from_toml_value(&toml::Value::Table(t)).unwrap_err();
    assert!(err.to_string().contains("table"));
}

#[test]
fn rejects_mixed_array() {
    let arr = toml::Value::Array(vec![
        toml::Value::String("ok".into()),
        toml::Value::Integer(7),
    ]);
    let err = ParameterValue::from_toml_value(&arr).unwrap_err();
    assert!(err.to_string().contains("list"));
}

#[test]
fn type_tag_strings() {
    assert_eq!(ParameterValue::String("x".into()).type_tag(), "string");
    assert_eq!(ParameterValue::Integer(0).type_tag(), "integer");
    assert_eq!(ParameterValue::Boolean(false).type_tag(), "boolean");
    assert_eq!(
        ParameterValue::ListString(vec![]).type_tag(),
        "list-of-string"
    );
}

#[test]
fn display_is_human_readable() {
    assert_eq!(format!("{}", ParameterValue::String("a".into())), "a");
    assert_eq!(format!("{}", ParameterValue::Integer(42)), "42");
    assert_eq!(format!("{}", ParameterValue::Boolean(true)), "true");
    assert_eq!(
        format!(
            "{}",
            ParameterValue::ListString(vec!["a".into(), "b".into()])
        ),
        "[\"a\", \"b\"]"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test parameter_value 2>&1 | tail -20`
Expected: FAIL with `unresolved import aenv_core::parameters`.

- [ ] **Step 3: Write minimal implementation**

Create `crates/aenv-core/src/parameters.rs`:

```rust
//! Typed parameters declared in a namespace's `[parameters]` table.
//!
//! Phase 3 supports four TOML types — string, integer, boolean, list-of-string
//! — and rejects everything else (`float`, `datetime`, `table`, mixed-type
//! arrays) at parse time. Adapters declare which parameters they consume via
//! `Adapter::parameters`; the resolver then enforces type-compat (R-71).

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A typed parameter value, parsed from a `[parameters]` table entry.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue {
    /// String value, e.g. `default_model = "claude-opus-4.7"`.
    String(String),
    /// Integer value, e.g. `instructions_budget = 3000`.
    Integer(i64),
    /// Boolean value, e.g. `auto_invoke_subagents = true`.
    Boolean(bool),
    /// Homogeneous list of strings, e.g. `forbid_tools = ["edit", "write"]`.
    ListString(Vec<String>),
}

impl ParameterValue {
    /// Convert a `toml::Value` into a `ParameterValue`, rejecting unsupported
    /// shapes. Returns `ManifestInvalid` with a human-readable reason.
    pub fn from_toml_value(v: &toml::Value) -> Result<Self> {
        match v {
            toml::Value::String(s) => Ok(ParameterValue::String(s.clone())),
            toml::Value::Integer(i) => Ok(ParameterValue::Integer(*i)),
            toml::Value::Boolean(b) => Ok(ParameterValue::Boolean(*b)),
            toml::Value::Array(arr) => {
                let mut out = Vec::with_capacity(arr.len());
                for (i, elem) in arr.iter().enumerate() {
                    match elem {
                        toml::Value::String(s) => out.push(s.clone()),
                        other => {
                            return Err(AenvError::ManifestInvalid(format!(
                                "parameter list element {i} is {} but only list-of-string is supported",
                                toml_type_name(other)
                            )));
                        }
                    }
                }
                Ok(ParameterValue::ListString(out))
            }
            toml::Value::Float(_) => Err(AenvError::ManifestInvalid(
                "parameter has float type; only string, integer, boolean, list-of-string are supported"
                    .into(),
            )),
            toml::Value::Datetime(_) => Err(AenvError::ManifestInvalid(
                "parameter has datetime type; only string, integer, boolean, list-of-string are supported"
                    .into(),
            )),
            toml::Value::Table(_) => Err(AenvError::ManifestInvalid(
                "parameter has table type; only string, integer, boolean, list-of-string are supported"
                    .into(),
            )),
        }
    }

    /// One of "string", "integer", "boolean", "list-of-string". Used in
    /// error messages and for type-compat checks against adapter declarations.
    pub fn type_tag(&self) -> &'static str {
        match self {
            ParameterValue::String(_) => "string",
            ParameterValue::Integer(_) => "integer",
            ParameterValue::Boolean(_) => "boolean",
            ParameterValue::ListString(_) => "list-of-string",
        }
    }
}

impl fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterValue::String(s) => write!(f, "{s}"),
            ParameterValue::Integer(i) => write!(f, "{i}"),
            ParameterValue::Boolean(b) => write!(f, "{b}"),
            ParameterValue::ListString(xs) => {
                let parts: Vec<String> = xs.iter().map(|s| format!("\"{s}\"")).collect();
                write!(f, "[{}]", parts.join(", "))
            }
        }
    }
}

fn toml_type_name(v: &toml::Value) -> &'static str {
    match v {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}
```

Add to `crates/aenv-core/src/lib.rs` after the existing `pub mod` lines:

```rust
pub mod parameters;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p aenv-core --test parameter_value 2>&1 | tail -20`
Expected: PASS — 10 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/parameters.rs crates/aenv-core/src/lib.rs crates/aenv-core/tests/parameter_value.rs
git commit -m "Add ParameterValue type with TOML parsing"
```

---

### Task 2: Extend `AenvManifest` with the `[parameters]` table

Now manifests can carry parameter values. We treat the TOML `[parameters]` table as a free-form `BTreeMap<String, toml::Value>` at the serde layer, then translate each entry through `ParameterValue::from_toml_value` at construction so type errors surface as `ManifestInvalid` (R-66, R-71 partial).

**Files:**
- Modify: `crates/aenv-core/src/manifest.rs`
- Test: `crates/aenv-core/tests/manifest_parameters.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/manifest_parameters.rs`:

```rust
use aenv_core::manifest::AenvManifest;
use aenv_core::parameters::ParameterValue;

#[test]
fn parses_all_four_types() {
    let toml = r#"
name = "detailed-execution"

[parameters]
default_model = "claude-opus-4.7"
instructions_budget = 3000
auto_invoke_subagents = true
forbid_tools = ["edit", "write"]
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(
        m.parameters.get("default_model"),
        Some(&ParameterValue::String("claude-opus-4.7".into()))
    );
    assert_eq!(
        m.parameters.get("instructions_budget"),
        Some(&ParameterValue::Integer(3000))
    );
    assert_eq!(
        m.parameters.get("auto_invoke_subagents"),
        Some(&ParameterValue::Boolean(true))
    );
    assert_eq!(
        m.parameters.get("forbid_tools"),
        Some(&ParameterValue::ListString(vec!["edit".into(), "write".into()]))
    );
}

#[test]
fn missing_block_is_empty_map() {
    let toml = r#"name = "base""#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert!(m.parameters.is_empty());
}

#[test]
fn rejects_float_value() {
    let toml = r#"
name = "x"
[parameters]
bad = 1.5
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("float"));
    assert!(err.to_string().contains("bad"));
}

#[test]
fn rejects_mixed_array() {
    let toml = r#"
name = "x"
[parameters]
bad = ["ok", 7]
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("list") || err.to_string().contains("bad"));
}

#[test]
fn roundtrip_preserves_parameters() {
    let toml = r#"
name = "x"

[parameters]
default_model = "claude-opus-4.7"
budget = 3000
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    let rendered = m.to_toml();
    let m2 = AenvManifest::from_toml(&rendered).unwrap();
    assert_eq!(m, m2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test manifest_parameters 2>&1 | tail -20`
Expected: FAIL — `m.parameters` does not exist.

- [ ] **Step 3: Modify `manifest.rs`**

In `crates/aenv-core/src/manifest.rs`, update the struct and `from_toml`. Replace the file body with:

```rust
//! Namespace manifest (`aenv.toml`) parsing.
//!
//! Phase 3 adds `[parameters]` and (Task 6) `[policies]`. Both tables go
//! through a two-stage parse: first into `toml::Value`, then each entry is
//! validated and converted into its typed shape. Type errors surface as
//! `ManifestInvalid` (exit 12).

use crate::error::{AenvError, Result};
use crate::parameters::ParameterValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A parsed namespace manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AenvManifest {
    /// Namespace name (must match the directory name; checked at activation time).
    pub name: String,

    /// Parent namespaces to inherit from. Resolution lives in Phase 2's
    /// `resolve::resolve_namespace`.
    #[serde(default)]
    pub extends: Vec<String>,

    /// Per-adapter configuration. Keys are adapter names (e.g. "claude-code").
    #[serde(default)]
    pub adapters: BTreeMap<String, AdapterEntry>,

    /// Typed parameters. Always non-`None` after a successful `from_toml`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterValue>,
}

/// Per-adapter manifest entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterEntry {
    /// Project-relative paths the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
    /// Per-file merge override. Key is relative path; value is one of:
    /// "section", "deep", "last-wins", "symlink".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<std::collections::BTreeMap<String, String>>,
}

impl AenvManifest {
    /// Parse a manifest from a TOML string. Two-stage: serde does the
    /// structural parse; then `[parameters]` entries are validated via
    /// `ParameterValue::from_toml_value`.
    pub fn from_toml(input: &str) -> Result<Self> {
        // Stage 1: structural parse into a raw shape that holds parameters as
        // `toml::Value` so we can validate per-entry.
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            #[serde(default)]
            extends: Vec<String>,
            #[serde(default)]
            adapters: BTreeMap<String, AdapterEntry>,
            #[serde(default)]
            parameters: BTreeMap<String, toml::Value>,
        }
        let raw: Raw =
            toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;

        // Stage 2: validate each parameter entry.
        let mut parameters: BTreeMap<String, ParameterValue> = BTreeMap::new();
        for (k, v) in &raw.parameters {
            let pv = ParameterValue::from_toml_value(v).map_err(|e| match e {
                AenvError::ManifestInvalid(reason) => {
                    AenvError::ManifestInvalid(format!("parameter '{k}': {reason}"))
                }
                other => other,
            })?;
            parameters.insert(k.clone(), pv);
        }

        Ok(AenvManifest {
            name: raw.name,
            extends: raw.extends,
            adapters: raw.adapters,
            parameters,
        })
    }

    /// Render the manifest to a canonical TOML string.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("AenvManifest serialization is infallible")
    }

    /// Build the manifest `aenv create <name>` writes by default.
    pub fn default_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            extends: Vec::new(),
            adapters: BTreeMap::new(),
            parameters: BTreeMap::new(),
        }
    }
}
```

- [ ] **Step 4: Run new test**

Run: `cargo test -p aenv-core --test manifest_parameters 2>&1 | tail -20`
Expected: PASS — 5 tests passed.

- [ ] **Step 5: Re-run Phase 2 tests (regression check)**

Run: `cargo test -p aenv-core 2>&1 | tail -20`
Expected: all prior tests still pass. The `Serialize` impl on `ParameterValue` is `#[serde(untagged)]` so existing TOML round-trips remain stable.

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/manifest.rs crates/aenv-core/tests/manifest_parameters.rs
git commit -m "Extend AenvManifest with [parameters] table"
```

---

### Task 3: Resolve parameter inheritance across the `extends` chain

Pure function over the already-resolved chain. Walks root→leaf and collapses every namespace's `[parameters]` table into a single `BTreeMap<String, ResolvedParameter>` with last-wins semantics per-key (R-67). Records the *source* namespace so `aenv get` can report provenance. Type-incompat across the chain (parent declares int, child declares string) aborts with `ManifestInvalid` (R-71 partial — the full R-71 check against adapter declarations is Task 5).

**Files:**
- Modify: `crates/aenv-core/src/parameters.rs`
- Test: `crates/aenv-core/tests/parameter_resolution.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/parameter_resolution.rs`:

```rust
use aenv_core::identity::NamespaceId;
use aenv_core::parameters::{resolve_parameters, ParameterValue, ResolvedParameter};
use std::collections::BTreeMap;

fn ns(name: &str) -> NamespaceId {
    NamespaceId::new(name).unwrap()
}

fn pv_string(s: &str) -> ParameterValue {
    ParameterValue::String(s.into())
}

#[test]
fn single_namespace_passes_through() {
    let chain = vec![ns("base")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("default_model".into(), pv_string("haiku"))]),
    );

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    let p = resolved.get("default_model").unwrap();
    assert_eq!(p.value, pv_string("haiku"));
    assert_eq!(p.source, ns("base"));
}

#[test]
fn child_overrides_parent() {
    let chain = vec![ns("base"), ns("detailed-execution")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("default_model".into(), pv_string("haiku"))]),
    );
    per_ns.insert(
        ns("detailed-execution"),
        BTreeMap::from([("default_model".into(), pv_string("opus"))]),
    );

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    let p = resolved.get("default_model").unwrap();
    assert_eq!(p.value, pv_string("opus"));
    assert_eq!(p.source, ns("detailed-execution"));
}

#[test]
fn parent_only_keys_pass_through() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("budget".into(), ParameterValue::Integer(5000))]),
    );
    per_ns.insert(ns("leaf"), BTreeMap::new());

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    let p = resolved.get("budget").unwrap();
    assert_eq!(p.value, ParameterValue::Integer(5000));
    assert_eq!(p.source, ns("base"));
}

#[test]
fn type_mismatch_across_chain_errors() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("budget".into(), ParameterValue::Integer(5000))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("budget".into(), pv_string("a lot"))]),
    );

    let err = resolve_parameters(&chain, &per_ns).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("budget"), "msg = {msg}");
    assert!(
        msg.contains("integer") && msg.contains("string"),
        "expected both type tags in msg, got: {msg}"
    );
}

#[test]
fn three_level_chain_last_wins() {
    let chain = vec![ns("root"), ns("mid"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("root"),
        BTreeMap::from([("model".into(), pv_string("haiku"))]),
    );
    per_ns.insert(
        ns("mid"),
        BTreeMap::from([("model".into(), pv_string("sonnet"))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("model".into(), pv_string("opus"))]),
    );

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    let p = resolved.get("model").unwrap();
    assert_eq!(p.value, pv_string("opus"));
    assert_eq!(p.source, ns("leaf"));
}

#[test]
fn unrelated_keys_dont_clash() {
    let chain = vec![ns("a"), ns("b")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>> = BTreeMap::new();
    per_ns.insert(
        ns("a"),
        BTreeMap::from([("x".into(), ParameterValue::Integer(1))]),
    );
    per_ns.insert(
        ns("b"),
        BTreeMap::from([("y".into(), ParameterValue::Boolean(true))]),
    );

    let resolved = resolve_parameters(&chain, &per_ns).unwrap();
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved.get("x").unwrap().source, ns("a"));
    assert_eq!(resolved.get("y").unwrap().source, ns("b"));
}

#[test]
fn fields_accessible_via_struct() {
    // Sanity: the `ResolvedParameter` API uses public `value` and `source`.
    let rp = ResolvedParameter {
        value: ParameterValue::Integer(42),
        source: ns("base"),
    };
    assert_eq!(rp.value, ParameterValue::Integer(42));
    assert_eq!(rp.source.as_str(), "base");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aenv-core --test parameter_resolution 2>&1 | tail -10`
Expected: FAIL — `resolve_parameters` and `ResolvedParameter` don't exist.

- [ ] **Step 3: Implement resolution**

Append to `crates/aenv-core/src/parameters.rs`:

```rust
use crate::identity::NamespaceId;
use std::collections::BTreeMap;

/// One resolved parameter: value + the namespace in the `extends` chain that
/// supplied it.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedParameter {
    /// Effective value after `extends`-chain resolution.
    pub value: ParameterValue,
    /// Latest namespace in the chain that declared this key.
    pub source: NamespaceId,
}

/// Collapse per-namespace `[parameters]` tables into a single map of effective
/// values. `chain` is in root → leaf order (the order `resolve_namespace`
/// produces). `per_ns` must contain an entry for every namespace in `chain`,
/// even if that entry is empty.
///
/// Semantics:
/// * Last-wins per-key (PRD R-67). The leaf overrides the root.
/// * Type-incompat across the chain (parent declares `int`, child declares
///   `string`) is a `ManifestInvalid` error. Same-type overrides are fine.
///
/// This function does NOT consult adapter declarations; that's Task 5
/// (full R-71 enforcement).
pub fn resolve_parameters(
    chain: &[NamespaceId],
    per_ns: &BTreeMap<NamespaceId, BTreeMap<String, ParameterValue>>,
) -> Result<BTreeMap<String, ResolvedParameter>> {
    let mut out: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    for ns in chain {
        let table = match per_ns.get(ns) {
            Some(t) => t,
            None => continue,
        };
        for (k, v) in table {
            if let Some(prev) = out.get(k) {
                if prev.value.type_tag() != v.type_tag() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "parameter '{}' has incompatible types across chain: \
                         {} declared {} but {} declared {}",
                        k,
                        prev.source,
                        prev.value.type_tag(),
                        ns,
                        v.type_tag()
                    )));
                }
            }
            out.insert(
                k.clone(),
                ResolvedParameter {
                    value: v.clone(),
                    source: ns.clone(),
                },
            );
        }
    }
    Ok(out)
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p aenv-core --test parameter_resolution 2>&1 | tail -10`
Expected: PASS — 7 tests passed.

- [ ] **Step 5: Regression**

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/parameters.rs crates/aenv-core/tests/parameter_resolution.rs
git commit -m "Add resolve_parameters with last-wins inheritance and type-compat check"
```

---

### Task 4: Adapter `[[parameters]]` declarations (declaration-only)

Adapters declare which parameters they consume and what TOML type those parameters must be (R-68 first half). Phase 3 only parses the declaration into `Adapter::parameters`; the actual *projection* — writing tool-specific config based on the parameter — is deferred to Phase 4 or later. This task adds the parsing and a getter; Task 5 wires the cross-check that rejects manifest values that disagree with the adapter's declared type (R-71 complete).

The declaration looks like:

```toml
# In an adapter TOML
name = "claude-code"
files = ["CLAUDE.md", ".claude/"]

[[parameters]]
name = "default_model"
type = "string"

[[parameters]]
name = "auto_invoke_subagents"
type = "list-of-string"
# (Optional, deferred-use) where this parameter ultimately projects to:
projects_to = ".claude/settings.json"
```

**Files:**
- Modify: `crates/aenv-core/src/adapter.rs`
- Test: `crates/aenv-core/tests/adapter_parameter_decls.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/adapter_parameter_decls.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterParameterType};

#[test]
fn parses_no_parameters_when_absent() {
    let toml = r#"
name = "minimal"
files = ["a"]
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert!(a.parameters.is_empty());
}

#[test]
fn parses_all_four_types() {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md"]

[[parameters]]
name = "default_model"
type = "string"

[[parameters]]
name = "instructions_budget"
type = "integer"

[[parameters]]
name = "auto_invoke_subagents"
type = "list-of-string"

[[parameters]]
name = "verbose"
type = "boolean"
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert_eq!(a.parameters.len(), 4);
    assert_eq!(a.parameters[0].name, "default_model");
    assert_eq!(a.parameters[0].r#type, AdapterParameterType::String);
    assert_eq!(a.parameters[1].r#type, AdapterParameterType::Integer);
    assert_eq!(a.parameters[2].r#type, AdapterParameterType::ListString);
    assert_eq!(a.parameters[3].r#type, AdapterParameterType::Boolean);
}

#[test]
fn rejects_unknown_type() {
    let toml = r#"
name = "x"
[[parameters]]
name = "y"
type = "float"
"#;
    let err = Adapter::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("float"));
}

#[test]
fn optional_projection_target_is_captured() {
    let toml = r#"
name = "claude-code"
[[parameters]]
name = "auto_invoke_subagents"
type = "list-of-string"
projects_to = ".claude/settings.json"
"#;
    let a = Adapter::from_toml(toml).unwrap();
    assert_eq!(a.parameters.len(), 1);
    assert_eq!(
        a.parameters[0].projects_to.as_deref(),
        Some(".claude/settings.json")
    );
}

#[test]
fn rejects_duplicate_name_within_adapter() {
    let toml = r#"
name = "x"
[[parameters]]
name = "dup"
type = "string"

[[parameters]]
name = "dup"
type = "integer"
"#;
    let err = Adapter::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("dup"));
}
```

- [ ] **Step 2: Verify it fails**

Run: `cargo test -p aenv-core --test adapter_parameter_decls 2>&1 | tail -10`
Expected: FAIL — `AdapterParameterType` and `Adapter::parameters` field do not exist.

- [ ] **Step 3: Extend `Adapter`**

Edit `crates/aenv-core/src/adapter.rs`. Add type and field; update `Adapter::from_toml` to enforce no-duplicates:

```rust
//! Adapter TOML parsing and registry.

use crate::error::{AenvError, Result};
use crate::fs::Filesystem;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Allowed parameter type for an adapter declaration.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterParameterType {
    /// `parameter = "..."`
    String,
    /// `parameter = 1234`
    Integer,
    /// `parameter = true`
    Boolean,
    /// `parameter = ["a", "b"]`
    ListString,
}

impl AdapterParameterType {
    /// String matching `ParameterValue::type_tag()`.
    pub fn type_tag(&self) -> &'static str {
        match self {
            AdapterParameterType::String => "string",
            AdapterParameterType::Integer => "integer",
            AdapterParameterType::Boolean => "boolean",
            AdapterParameterType::ListString => "list-of-string",
        }
    }
}

/// One parameter an adapter consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterParameterDecl {
    /// Parameter key (e.g. `"default_model"`).
    pub name: String,
    /// Expected TOML type.
    #[serde(rename = "type")]
    pub r#type: AdapterParameterType,
    /// Optional projection target. Phase 3 records this; Phase 4+ uses it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects_to: Option<String>,
}

/// A parsed adapter definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adapter {
    /// Adapter name (e.g. "claude-code").
    pub name: String,
    /// Project-relative paths or directory prefixes the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
    /// Phase 1 holdover — explicit per-file merge declaration on the adapter.
    #[serde(default)]
    pub merge_strategies: BTreeMap<String, String>,
    /// Per-path role declaration. Phase 2 understands `"instructions"`.
    #[serde(default)]
    pub roles: BTreeMap<String, String>,
    /// Per-path default merge strategy (consulted before role fallback).
    #[serde(default)]
    pub default_merge: BTreeMap<String, String>,
    /// Parameters this adapter consumes. Empty for adapters that take none.
    #[serde(default, rename = "parameters", skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<AdapterParameterDecl>,
}

impl Adapter {
    /// Parse an adapter from a TOML string. Enforces no-duplicate parameter
    /// names within a single adapter.
    pub fn from_toml(input: &str) -> Result<Self> {
        let a: Adapter =
            toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for p in &a.parameters {
            if !seen.insert(p.name.as_str()) {
                return Err(AenvError::ManifestInvalid(format!(
                    "adapter '{}' declares parameter '{}' more than once",
                    a.name, p.name
                )));
            }
        }
        Ok(a)
    }
}

/// In-memory set of loaded adapters, keyed by name.
#[derive(Debug, Default, Clone)]
pub struct AdapterRegistry {
    adapters: BTreeMap<String, Adapter>,
}

impl AdapterRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of adapters loaded.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Add an adapter to the registry.
    pub fn insert(&mut self, adapter: Adapter) {
        self.adapters.insert(adapter.name.clone(), adapter);
    }

    /// Look up an adapter by name.
    pub fn get(&self, name: &str) -> Option<&Adapter> {
        self.adapters.get(name)
    }

    /// Iterate over all adapters.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Adapter)> {
        self.adapters.iter()
    }

    /// Load every `.toml` file from `dir` into a registry. Non-TOML files
    /// are silently skipped. A missing `dir` returns an empty registry.
    pub fn load_from_dir<F: Filesystem>(fs: &F, dir: &Path) -> Result<Self> {
        let mut reg = Self::new();
        if !fs.exists(dir)? {
            return Ok(reg);
        }
        for path in fs.list_dir(dir)? {
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            let bytes = fs.read(&path)?;
            let toml_str = std::str::from_utf8(&bytes).map_err(|e| {
                AenvError::ManifestInvalid(format!("{}: not utf-8: {e}", path.display()))
            })?;
            reg.insert(Adapter::from_toml(toml_str)?);
        }
        Ok(reg)
    }
}
```

- [ ] **Step 4: Run new + existing tests**

Run: `cargo test -p aenv-core --test adapter_parameter_decls 2>&1 | tail -10`
Expected: PASS — 5 tests passed.

Run: `cargo test -p aenv-core --test adapter 2>&1 | tail -10`
Expected: still PASS — Phase 1 adapter tests should not be disturbed by the new `parameters` field (it defaults to empty).

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/adapter.rs crates/aenv-core/tests/adapter_parameter_decls.rs
git commit -m "Add adapter [[parameters]] declarations (declaration-only)"
```

---

### Task 5: Type-compat check across adapter declarations (R-71 complete)

`R-71` says: "If a parameter declared by an adapter is referenced by a manifest with a type-incompatible value, the system shall report a manifest-invalid error and refuse to activate." With Tasks 2–4 we now have everything we need: a resolved parameter map (Task 3) and an adapter registry whose adapters declare expected types (Task 4). This task adds the cross-check.

**Semantics:**

- If an adapter declares parameter `p` with type `T`, and the resolved parameter map contains `p` with type `T'` ≠ `T`, that's `ManifestInvalid` (exit 12).
- If multiple adapters in the resolution declare the same parameter name, they must agree on the type (otherwise it's a configuration bug; abort with `ManifestInvalid`).
- A parameter declared by no adapter is allowed (PRD §4.4 shows `forbid_tools` — used by downstream tooling, no adapter declares it). The check is one-way: adapter-declared → manifest must match. Manifest-only parameters fall through silently.

**Files:**
- Modify: `crates/aenv-core/src/parameters.rs` — add `check_against_adapters`
- Test: `crates/aenv-core/tests/parameter_adapter_check.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/parameter_adapter_check.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterParameterDecl, AdapterParameterType, AdapterRegistry};
use aenv_core::identity::NamespaceId;
use aenv_core::parameters::{check_against_adapters, ParameterValue, ResolvedParameter};
use std::collections::BTreeMap;

fn ns(name: &str) -> NamespaceId {
    NamespaceId::new(name).unwrap()
}

fn registry_with(adapters: Vec<Adapter>) -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    for a in adapters {
        r.insert(a);
    }
    r
}

fn ad(name: &str, params: Vec<(&str, AdapterParameterType)>) -> Adapter {
    Adapter {
        name: name.into(),
        files: vec![],
        merge_strategies: BTreeMap::new(),
        roles: BTreeMap::new(),
        default_merge: BTreeMap::new(),
        parameters: params
            .into_iter()
            .map(|(n, t)| AdapterParameterDecl {
                name: n.into(),
                r#type: t,
                projects_to: None,
            })
            .collect(),
    }
}

fn rp(value: ParameterValue, source: &str) -> ResolvedParameter {
    ResolvedParameter {
        value,
        source: ns(source),
    }
}

#[test]
fn passes_when_types_match() {
    let registry = registry_with(vec![ad(
        "claude-code",
        vec![("default_model", AdapterParameterType::String)],
    )]);
    let mut resolved: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    resolved.insert(
        "default_model".into(),
        rp(ParameterValue::String("opus".into()), "leaf"),
    );

    check_against_adapters(&resolved, &registry).unwrap();
}

#[test]
fn fails_when_manifest_type_disagrees_with_adapter() {
    let registry = registry_with(vec![ad(
        "claude-code",
        vec![("default_model", AdapterParameterType::String)],
    )]);
    let mut resolved: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    resolved.insert(
        "default_model".into(),
        rp(ParameterValue::Integer(42), "leaf"),
    );

    let err = check_against_adapters(&resolved, &registry).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("default_model"), "msg = {msg}");
    assert!(msg.contains("string"), "msg = {msg}");
    assert!(msg.contains("integer"), "msg = {msg}");
}

#[test]
fn allows_parameters_not_declared_by_any_adapter() {
    // `forbid_tools` is consumed by downstream tooling; no adapter declares it.
    let registry = registry_with(vec![ad(
        "claude-code",
        vec![("default_model", AdapterParameterType::String)],
    )]);
    let mut resolved: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    resolved.insert(
        "forbid_tools".into(),
        rp(
            ParameterValue::ListString(vec!["edit".into(), "write".into()]),
            "leaf",
        ),
    );

    check_against_adapters(&resolved, &registry).unwrap();
}

#[test]
fn rejects_conflicting_adapter_declarations() {
    let registry = registry_with(vec![
        ad("a", vec![("x", AdapterParameterType::String)]),
        ad("b", vec![("x", AdapterParameterType::Integer)]),
    ]);
    let mut resolved: BTreeMap<String, ResolvedParameter> = BTreeMap::new();
    resolved.insert("x".into(), rp(ParameterValue::String("v".into()), "leaf"));

    let err = check_against_adapters(&resolved, &registry).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("'x'"), "msg = {msg}");
    assert!(msg.contains('a') && msg.contains('b'), "msg = {msg}");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test parameter_adapter_check 2>&1 | tail -10`
Expected: FAIL — `check_against_adapters` doesn't exist.

- [ ] **Step 3: Implement the check**

Append to `crates/aenv-core/src/parameters.rs`:

```rust
use crate::adapter::AdapterRegistry;

/// Verify that every adapter-declared parameter has a manifest value of the
/// adapter-declared type (PRD R-71). Manifest-only parameters (not declared
/// by any adapter) are allowed.
///
/// Also rejects the case where two adapters declare the same parameter name
/// with different types — that's a configuration bug.
pub fn check_against_adapters(
    resolved: &BTreeMap<String, ResolvedParameter>,
    adapters: &AdapterRegistry,
) -> Result<()> {
    // Build a map: parameter name -> (adapter_name, type_tag).
    let mut decls: BTreeMap<&str, (&str, &'static str)> = BTreeMap::new();
    for (adapter_name, adapter) in adapters.iter() {
        for p in &adapter.parameters {
            if let Some((other_adapter, other_type)) = decls.get(p.name.as_str()) {
                if *other_type != p.r#type.type_tag() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "parameter '{}' is declared by adapters '{}' ({}) \
                         and '{}' ({}) with conflicting types",
                        p.name,
                        other_adapter,
                        other_type,
                        adapter_name,
                        p.r#type.type_tag()
                    )));
                }
            } else {
                decls.insert(p.name.as_str(), (adapter_name.as_str(), p.r#type.type_tag()));
            }
        }
    }

    for (name, rp) in resolved {
        if let Some((adapter_name, decl_type)) = decls.get(name.as_str()) {
            if *decl_type != rp.value.type_tag() {
                return Err(AenvError::ManifestInvalid(format!(
                    "parameter '{}' has type {} in namespace {} but adapter '{}' \
                     declared it as {}",
                    name,
                    rp.value.type_tag(),
                    rp.source,
                    adapter_name,
                    decl_type
                )));
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p aenv-core --test parameter_adapter_check 2>&1 | tail -10`
Expected: PASS — 4 tests passed.

- [ ] **Step 5: Regression**

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/parameters.rs crates/aenv-core/tests/parameter_adapter_check.rs
git commit -m "Add R-71 adapter/manifest parameter type-compat check"
```

---

### Task 6: Policy primitives + `[policies]` manifest parsing

Two value shapes for a policy:

- **Shorthand:** `instructions_max_chars = 3000` — value with `enforce = false` (advisory).
- **Table form:** `instructions_max_chars = { value = 3000, enforce = true }` — explicit enforce flag.

`PolicyValue` is integer | boolean | list-of-string. (String is not used for any known policy key, and admitting it now would just sprawl the type-compat surface. We reject string-valued policies at parse time.)

Unknown policy keys parse successfully (we don't gate on a known-key allowlist) but `aenv doctor` skips them with a warning (Task 16). This lets adapter-specific keys land later without re-parsing every manifest.

**Files:**
- Create: `crates/aenv-core/src/policies.rs`
- Modify: `crates/aenv-core/src/lib.rs` (add `pub mod policies;`)
- Modify: `crates/aenv-core/src/manifest.rs` (`AenvManifest` gains `policies: BTreeMap<String, PolicyDecl>`)
- Test: `crates/aenv-core/tests/manifest_policies.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/manifest_policies.rs`:

```rust
use aenv_core::manifest::AenvManifest;
use aenv_core::policies::{PolicyDecl, PolicyValue};

#[test]
fn parses_shorthand_advisory() {
    let toml = r#"
name = "base"

[policies]
instructions_max_chars = 5000
skill_requires_description = true
forbid_paths = [".env*", "secrets/**"]
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(
        m.policies.get("instructions_max_chars"),
        Some(&PolicyDecl {
            value: PolicyValue::Integer(5000),
            enforce: false,
        })
    );
    assert_eq!(
        m.policies.get("skill_requires_description"),
        Some(&PolicyDecl {
            value: PolicyValue::Boolean(true),
            enforce: false,
        })
    );
    assert_eq!(
        m.policies.get("forbid_paths"),
        Some(&PolicyDecl {
            value: PolicyValue::ListString(vec![".env*".into(), "secrets/**".into()]),
            enforce: false,
        })
    );
}

#[test]
fn parses_table_form_enforce() {
    let toml = r#"
name = "leaf"

[policies]
instructions_max_chars = { value = 3000, enforce = true }
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(
        m.policies.get("instructions_max_chars"),
        Some(&PolicyDecl {
            value: PolicyValue::Integer(3000),
            enforce: true,
        })
    );
}

#[test]
fn parses_table_form_explicit_advisory() {
    let toml = r#"
name = "x"

[policies]
forbid_paths = { value = ["a"], enforce = false }
"#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert_eq!(
        m.policies.get("forbid_paths").unwrap().enforce,
        false
    );
}

#[test]
fn missing_block_is_empty_map() {
    let toml = r#"name = "x""#;
    let m = AenvManifest::from_toml(toml).unwrap();
    assert!(m.policies.is_empty());
}

#[test]
fn rejects_string_value() {
    let toml = r#"
name = "x"

[policies]
weird = "this is not a valid policy value"
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("weird"));
}

#[test]
fn rejects_table_without_value_field() {
    let toml = r#"
name = "x"

[policies]
bad = { enforce = true }
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(err.to_string().contains("bad"));
}

#[test]
fn rejects_mixed_list_value() {
    let toml = r#"
name = "x"

[policies]
forbid_paths = ["ok", 5]
"#;
    let err = AenvManifest::from_toml(toml).unwrap_err();
    assert!(
        err.to_string().contains("forbid_paths") || err.to_string().contains("list"),
        "msg = {err}"
    );
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test manifest_policies 2>&1 | tail -10`
Expected: FAIL — `aenv_core::policies` doesn't exist.

- [ ] **Step 3: Create `policies.rs`**

Create `crates/aenv-core/src/policies.rs`:

```rust
//! Policy declarations on a namespace manifest.
//!
//! `[policies]` accepts two shapes per key:
//!
//! ```toml
//! [policies]
//! instructions_max_chars = 3000                                    # advisory
//! skill_requires_description = { value = true, enforce = true }    # enforced
//! ```
//!
//! Phase 3 understands four built-in policy keys (`instructions_max_chars`,
//! `skill_requires_description`, `mcp_requires_command_or_url`, `forbid_paths`).
//! Unknown keys parse but are skipped by the evaluator (`aenv doctor` emits
//! a warning).

use crate::error::{AenvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Value side of a policy declaration. Integer, boolean, or list-of-string.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolicyValue {
    /// Integer policy value (e.g. `instructions_max_chars = 3000`).
    Integer(i64),
    /// Boolean policy value (e.g. `skill_requires_description = true`).
    Boolean(bool),
    /// List-of-string policy value (e.g. `forbid_paths = ["secrets/**"]`).
    ListString(Vec<String>),
}

impl PolicyValue {
    /// One of "integer", "boolean", "list-of-string".
    pub fn type_tag(&self) -> &'static str {
        match self {
            PolicyValue::Integer(_) => "integer",
            PolicyValue::Boolean(_) => "boolean",
            PolicyValue::ListString(_) => "list-of-string",
        }
    }
}

/// One policy declaration in a manifest's `[policies]` table.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolicyDecl {
    /// The policy's value (type depends on the key).
    pub value: PolicyValue,
    /// `enforce = true` makes activation refuse on violation.
    #[serde(default)]
    pub enforce: bool,
}

impl PolicyDecl {
    /// Convert a `toml::Value` (which may be a value or a `{ value, enforce }`
    /// table) into a `PolicyDecl`. Returns `ManifestInvalid` for unsupported
    /// shapes.
    pub fn from_toml_value(key: &str, v: &toml::Value) -> Result<Self> {
        if let toml::Value::Table(t) = v {
            // Table form: must have `value`; `enforce` defaults to false.
            let value_tv = t.get("value").ok_or_else(|| {
                AenvError::ManifestInvalid(format!(
                    "policy '{key}' table-form is missing 'value' field"
                ))
            })?;
            let value = policy_value_from_toml(key, value_tv)?;
            let enforce = match t.get("enforce") {
                Some(toml::Value::Boolean(b)) => *b,
                Some(other) => {
                    return Err(AenvError::ManifestInvalid(format!(
                        "policy '{key}' has non-boolean 'enforce' field ({})",
                        toml_type_name(other)
                    )));
                }
                None => false,
            };
            // Reject any other unexpected fields in the table to surface typos.
            for k in t.keys() {
                if k != "value" && k != "enforce" {
                    return Err(AenvError::ManifestInvalid(format!(
                        "policy '{key}' has unknown field '{k}' (only 'value' and 'enforce' are accepted)"
                    )));
                }
            }
            Ok(PolicyDecl { value, enforce })
        } else {
            // Shorthand: bare value, advisory.
            Ok(PolicyDecl {
                value: policy_value_from_toml(key, v)?,
                enforce: false,
            })
        }
    }
}

fn policy_value_from_toml(key: &str, v: &toml::Value) -> Result<PolicyValue> {
    match v {
        toml::Value::Integer(i) => Ok(PolicyValue::Integer(*i)),
        toml::Value::Boolean(b) => Ok(PolicyValue::Boolean(*b)),
        toml::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for (i, elem) in arr.iter().enumerate() {
                match elem {
                    toml::Value::String(s) => out.push(s.clone()),
                    other => {
                        return Err(AenvError::ManifestInvalid(format!(
                            "policy '{key}' list element {i} is {} but only list-of-string is supported",
                            toml_type_name(other)
                        )));
                    }
                }
            }
            Ok(PolicyValue::ListString(out))
        }
        other => Err(AenvError::ManifestInvalid(format!(
            "policy '{key}' has unsupported value type {}; \
             only integer, boolean, list-of-string (or {{ value = ..., enforce = bool }}) are supported",
            toml_type_name(other)
        ))),
    }
}

fn toml_type_name(v: &toml::Value) -> &'static str {
    match v {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}

/// Convenience: parse every entry in a `BTreeMap<String, toml::Value>`
/// (typically from a manifest's `[policies]` table) into typed `PolicyDecl`s.
pub fn parse_policy_table(
    raw: &BTreeMap<String, toml::Value>,
) -> Result<BTreeMap<String, PolicyDecl>> {
    let mut out = BTreeMap::new();
    for (k, v) in raw {
        out.insert(k.clone(), PolicyDecl::from_toml_value(k, v)?);
    }
    Ok(out)
}
```

Add to `crates/aenv-core/src/lib.rs`:

```rust
pub mod policies;
```

Now extend `AenvManifest`. Modify the `Raw` struct inside `from_toml` to include `policies` and post-process them through `parse_policy_table`:

```rust
// In crates/aenv-core/src/manifest.rs, replace the existing struct and from_toml:

use crate::parameters::ParameterValue;
use crate::policies::{parse_policy_table, PolicyDecl};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AenvManifest {
    pub name: String,
    #[serde(default)]
    pub extends: Vec<String>,
    #[serde(default)]
    pub adapters: BTreeMap<String, AdapterEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub policies: BTreeMap<String, PolicyDecl>,
}

impl AenvManifest {
    pub fn from_toml(input: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            #[serde(default)]
            extends: Vec<String>,
            #[serde(default)]
            adapters: BTreeMap<String, AdapterEntry>,
            #[serde(default)]
            parameters: BTreeMap<String, toml::Value>,
            #[serde(default)]
            policies: BTreeMap<String, toml::Value>,
        }
        let raw: Raw =
            toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;

        let mut parameters: BTreeMap<String, ParameterValue> = BTreeMap::new();
        for (k, v) in &raw.parameters {
            let pv = ParameterValue::from_toml_value(v).map_err(|e| match e {
                AenvError::ManifestInvalid(reason) => {
                    AenvError::ManifestInvalid(format!("parameter '{k}': {reason}"))
                }
                other => other,
            })?;
            parameters.insert(k.clone(), pv);
        }

        let policies = parse_policy_table(&raw.policies)?;

        Ok(AenvManifest {
            name: raw.name,
            extends: raw.extends,
            adapters: raw.adapters,
            parameters,
            policies,
        })
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("AenvManifest serialization is infallible")
    }

    pub fn default_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            extends: Vec::new(),
            adapters: BTreeMap::new(),
            parameters: BTreeMap::new(),
            policies: BTreeMap::new(),
        }
    }
}
```

- [ ] **Step 4: Run new + existing tests**

Run: `cargo test -p aenv-core --test manifest_policies 2>&1 | tail -10`
Expected: PASS — 7 tests passed.

Run: `cargo test -p aenv-core --test manifest_parameters 2>&1 | tail -10`
Expected: still PASS.

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/policies.rs crates/aenv-core/src/lib.rs crates/aenv-core/src/manifest.rs crates/aenv-core/tests/manifest_policies.rs
git commit -m "Add PolicyDecl + [policies] manifest parsing"
```

---

### Task 7: Resolve policy inheritance with enforce-protection (R-75)

Like parameter resolution, but with a twist: a child cannot weaken a parent's `enforce = true` declaration (R-75). "Weaken" is defined precisely so the implementation isn't a judgment call:

| Type | "Weaker" means |
|---|---|
| Integer | The child raises the limit (e.g. parent `instructions_max_chars = 3000`; child `instructions_max_chars = 5000` → weakens). |
| Boolean | The child sets `false` where the parent set `true`, OR clears `enforce`. |
| ListString | The child's list is missing one or more entries the parent had (a deny-list is shorter). |

Same-or-stricter overrides succeed (with `source` set to the child); strictly weakening overrides fail with `ManifestInvalid` (exit 12 — *not* `PolicyViolation`, because the violation is in the manifest, not in the artifacts). Removing the key entirely from the child is treated as inheriting unchanged.

A child may *upgrade* a parent's advisory policy to `enforce = true`. A child may always tighten (lower the int limit, expand a deny-list, flip `false` → `true`).

**Files:**
- Modify: `crates/aenv-core/src/policies.rs` — add `ResolvedPolicy`, `resolve_policies`
- Test: `crates/aenv-core/tests/policy_resolution.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/policy_resolution.rs`:

```rust
use aenv_core::identity::NamespaceId;
use aenv_core::policies::{resolve_policies, PolicyDecl, PolicyValue, ResolvedPolicy};
use std::collections::BTreeMap;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn pd_int(i: i64, enforce: bool) -> PolicyDecl {
    PolicyDecl {
        value: PolicyValue::Integer(i),
        enforce,
    }
}

fn pd_bool(b: bool, enforce: bool) -> PolicyDecl {
    PolicyDecl {
        value: PolicyValue::Boolean(b),
        enforce,
    }
}

fn pd_list(xs: &[&str], enforce: bool) -> PolicyDecl {
    PolicyDecl {
        value: PolicyValue::ListString(xs.iter().map(|s| (*s).into()).collect()),
        enforce,
    }
}

#[test]
fn single_namespace_passes_through() {
    let chain = vec![ns("base")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(5000, false))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    let p = resolved.get("k").unwrap();
    assert_eq!(p.value, PolicyValue::Integer(5000));
    assert_eq!(p.enforce, false);
    assert_eq!(p.source, ns("base"));
}

#[test]
fn child_advisory_override_wins() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(5000, false))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_int(3000, false))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    let p = resolved.get("k").unwrap();
    assert_eq!(p.value, PolicyValue::Integer(3000));
    assert_eq!(p.source, ns("leaf"));
}

#[test]
fn child_can_upgrade_advisory_to_enforce() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(5000, false))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_int(3000, true))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    let p = resolved.get("k").unwrap();
    assert_eq!(p.enforce, true);
    assert_eq!(p.source, ns("leaf"));
}

#[test]
fn child_cannot_downgrade_enforce_to_advisory() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(3000, true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_int(3000, false))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    assert!(err.to_string().contains("'k'"));
    assert!(err.to_string().contains("enforce"));
}

#[test]
fn child_cannot_raise_enforced_int_limit() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("instructions_max_chars".into(), pd_int(3000, true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("instructions_max_chars".into(), pd_int(5000, true))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("instructions_max_chars"));
    assert!(msg.contains("weaken") || msg.contains("raise"));
}

#[test]
fn child_can_lower_enforced_int_limit() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_int(5000, true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_int(3000, true))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    assert_eq!(resolved.get("k").unwrap().value, PolicyValue::Integer(3000));
}

#[test]
fn child_cannot_flip_enforced_true_to_false() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("k".into(), pd_bool(true, true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_bool(false, true))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    assert!(err.to_string().contains("'k'"));
}

#[test]
fn child_cannot_shrink_enforced_deny_list() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([(
            "forbid_paths".into(),
            pd_list(&["secrets/**", ".env*"], true),
        )]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("forbid_paths".into(), pd_list(&[".env*"], true))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    assert!(err.to_string().contains("forbid_paths"));
    assert!(err.to_string().contains("secrets"));
}

#[test]
fn child_can_extend_enforced_deny_list() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(
        ns("base"),
        BTreeMap::from([("forbid_paths".into(), pd_list(&["a"], true))]),
    );
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("forbid_paths".into(), pd_list(&["a", "b"], true))]),
    );
    let resolved = resolve_policies(&chain, &per_ns).unwrap();
    let p = resolved.get("forbid_paths").unwrap();
    let xs = match &p.value {
        PolicyValue::ListString(xs) => xs.clone(),
        _ => panic!(),
    };
    assert!(xs.contains(&"a".to_string()));
    assert!(xs.contains(&"b".to_string()));
}

#[test]
fn type_mismatch_across_chain_errors() {
    let chain = vec![ns("base"), ns("leaf")];
    let mut per_ns: BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>> = BTreeMap::new();
    per_ns.insert(ns("base"), BTreeMap::from([("k".into(), pd_int(1, false))]));
    per_ns.insert(
        ns("leaf"),
        BTreeMap::from([("k".into(), pd_bool(false, false))]),
    );
    let err = resolve_policies(&chain, &per_ns).unwrap_err();
    assert!(err.to_string().contains("'k'"));
    assert!(err.to_string().contains("integer"));
    assert!(err.to_string().contains("boolean"));
}

#[test]
fn fields_accessible_via_struct() {
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(42),
        enforce: true,
        source: ns("base"),
    };
    assert_eq!(rp.value, PolicyValue::Integer(42));
    assert!(rp.enforce);
    assert_eq!(rp.source.as_str(), "base");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test policy_resolution 2>&1 | tail -10`
Expected: FAIL — `resolve_policies` and `ResolvedPolicy` don't exist.

- [ ] **Step 3: Implement resolution**

Append to `crates/aenv-core/src/policies.rs`:

```rust
use crate::identity::NamespaceId;

/// One resolved policy.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedPolicy {
    /// Effective value after `extends`-chain resolution.
    pub value: PolicyValue,
    /// Final `enforce` flag (`true` if any namespace in the chain set it).
    pub enforce: bool,
    /// Latest namespace in the chain that declared this key.
    pub source: NamespaceId,
}

/// Resolve `[policies]` tables across the `extends` chain. Returns
/// `ManifestInvalid` if any child weakens a parent's `enforce = true`
/// declaration (R-75) or if the same key has incompatible types across the chain.
///
/// "Weaken" is defined per type:
/// * Integer: raising the limit (`child > parent`) weakens.
/// * Boolean: flipping `true` -> `false` weakens.
/// * ListString: removing entries weakens (the child must be a superset).
/// * Enforce flag: changing `true` -> `false` weakens, even with the same value.
pub fn resolve_policies(
    chain: &[NamespaceId],
    per_ns: &BTreeMap<NamespaceId, BTreeMap<String, PolicyDecl>>,
) -> Result<BTreeMap<String, ResolvedPolicy>> {
    let mut out: BTreeMap<String, ResolvedPolicy> = BTreeMap::new();
    for ns in chain {
        let table = match per_ns.get(ns) {
            Some(t) => t,
            None => continue,
        };
        for (k, decl) in table {
            if let Some(prev) = out.get(k) {
                if prev.value.type_tag() != decl.value.type_tag() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "policy '{}' has incompatible types across chain: \
                         {} declared {} but {} declared {}",
                        k,
                        prev.source,
                        prev.value.type_tag(),
                        ns,
                        decl.value.type_tag()
                    )));
                }
                if prev.enforce {
                    enforce_protection(k, ns, prev, decl)?;
                }
            }
            out.insert(
                k.clone(),
                ResolvedPolicy {
                    value: decl.value.clone(),
                    enforce: decl.enforce,
                    source: ns.clone(),
                },
            );
        }
    }
    Ok(out)
}

fn enforce_protection(
    key: &str,
    child_ns: &NamespaceId,
    parent: &ResolvedPolicy,
    child: &PolicyDecl,
) -> Result<()> {
    // The parent is enforced. The child may keep enforce on or raise it; it
    // may not downgrade to advisory.
    if !child.enforce {
        return Err(AenvError::ManifestInvalid(format!(
            "policy '{}' is enforced by {} but {} sets enforce = false (R-75: \
             a child may not downgrade an inherited enforced policy)",
            key, parent.source, child_ns
        )));
    }

    // Same-or-stricter check by type.
    match (&parent.value, &child.value) {
        (PolicyValue::Integer(p), PolicyValue::Integer(c)) => {
            if c > p {
                return Err(AenvError::ManifestInvalid(format!(
                    "policy '{key}' is enforced by {} at {p}; {} attempts to weaken \
                     by raising the limit to {c} (R-75)",
                    parent.source, child_ns
                )));
            }
        }
        (PolicyValue::Boolean(p), PolicyValue::Boolean(c)) => {
            if *p && !*c {
                return Err(AenvError::ManifestInvalid(format!(
                    "policy '{key}' is enforced by {} at true; {} attempts to weaken \
                     to false (R-75)",
                    parent.source, child_ns
                )));
            }
        }
        (PolicyValue::ListString(p_list), PolicyValue::ListString(c_list)) => {
            for parent_entry in p_list {
                if !c_list.contains(parent_entry) {
                    return Err(AenvError::ManifestInvalid(format!(
                        "policy '{key}' is enforced by {} and includes '{parent_entry}'; \
                         {} attempts to weaken by removing it (R-75)",
                        parent.source, child_ns
                    )));
                }
            }
        }
        // Type-mismatch already caught by the caller.
        _ => unreachable!("type-mismatch should have been caught earlier"),
    }
    Ok(())
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p aenv-core --test policy_resolution 2>&1 | tail -10`
Expected: PASS — 11 tests passed.

- [ ] **Step 5: Regression**

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/policies.rs crates/aenv-core/tests/policy_resolution.rs
git commit -m "Add resolve_policies with R-75 enforce-protection"
```

---

### Task 8: `PolicyEvaluator` trait + dispatch scaffold

Each built-in policy is a separate evaluator that produces zero or more `PolicyOutcome`s for a given namespace and its on-disk artifacts. Tasks 9–12 implement the four evaluators. This task sets up the trait, the outcome shape, the dispatch table, and the "unknown key" warning path.

**Files:**
- Create: `crates/aenv-core/src/policies/builtin/mod.rs`
- Modify: `crates/aenv-core/src/policies.rs` (re-export the `builtin` submodule)
- Modify: `crates/aenv-core/src/lib.rs` (no change — `policies` is already there)
- Test: `crates/aenv-core/tests/policy_evaluator_scaffold.rs`

Note: this requires moving `policies.rs` to `policies/mod.rs` so it can contain a submodule. Phase 2's `merge/` directory did the same dance — follow that pattern.

- [ ] **Step 1: Move `policies.rs` to `policies/mod.rs`**

```bash
mkdir -p crates/aenv-core/src/policies
git mv crates/aenv-core/src/policies.rs crates/aenv-core/src/policies/mod.rs
```

Verify nothing else broke:

```bash
cargo build -p aenv-core 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 2: Write the failing test**

Create `crates/aenv-core/tests/policy_evaluator_scaffold.rs`:

```rust
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::policies::builtin::{
    dispatch, OutcomeStatus, PolicyContext, PolicyEvaluator, PolicyOutcome,
};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};

#[test]
fn unknown_key_returns_warn_skip() {
    let ctx = PolicyContext::dummy();
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: NamespaceId::new("base").unwrap(),
    };
    let out = dispatch("does_not_exist", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::WarnSkip { .. }));
    assert_eq!(out[0].key, "does_not_exist");
}

#[test]
fn outcome_struct_shape() {
    let o = PolicyOutcome {
        key: "k".into(),
        target: Some(QualifiedName::new(
            NamespaceId::new("base").unwrap(),
            ShortName::new("CLAUDE.md").unwrap(),
        )),
        status: OutcomeStatus::Pass,
    };
    assert_eq!(o.key, "k");
    assert!(o.target.is_some());
}

#[test]
fn pass_constructor_helper() {
    let o = PolicyOutcome::pass("k", None);
    assert!(matches!(o.status, OutcomeStatus::Pass));
}

#[test]
fn fail_constructor_helper() {
    let o = PolicyOutcome::fail("k", None, "reason");
    if let OutcomeStatus::Fail { msg } = &o.status {
        assert_eq!(msg, "reason");
    } else {
        panic!("expected Fail");
    }
}

#[test]
fn warn_constructor_helper() {
    let o = PolicyOutcome::warn("k", None, "hint");
    if let OutcomeStatus::Warn { msg } = &o.status {
        assert_eq!(msg, "hint");
    } else {
        panic!("expected Warn");
    }
}
```

- [ ] **Step 3: Verify failure**

Run: `cargo test -p aenv-core --test policy_evaluator_scaffold 2>&1 | tail -10`
Expected: FAIL — `policies::builtin` doesn't exist.

- [ ] **Step 4: Create `policies/builtin/mod.rs`**

Create `crates/aenv-core/src/policies/builtin/mod.rs`:

```rust
//! Built-in policy evaluators.
//!
//! Each built-in policy key (`instructions_max_chars`,
//! `skill_requires_description`, `mcp_requires_command_or_url`,
//! `forbid_paths`) ships as a dedicated evaluator. The `dispatch` function
//! routes a resolved policy to its evaluator; unknown keys produce a single
//! `WarnSkip` outcome so `aenv doctor` can report them without failing.
//!
//! `PolicyContext` carries the references an evaluator needs without forcing
//! every evaluator to take a long argument list.

use crate::adapter::AdapterRegistry;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::identity::QualifiedName;
use crate::policies::ResolvedPolicy;
use crate::resolve::ResolutionResult;

/// References an evaluator needs to walk the namespace + its artifacts.
pub struct PolicyContext<'a, F: Filesystem> {
    /// Filesystem the evaluator should read through.
    pub fs: &'a F,
    /// Registry layout (paths to namespace dirs, manifest paths).
    pub layout: &'a RegistryLayout,
    /// Adapter registry (for `role = "instructions"` lookup).
    pub adapters: &'a AdapterRegistry,
    /// The resolved chain whose artifacts we evaluate.
    pub resolved: &'a ResolutionResult,
}

/// One result emitted by an evaluator.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PolicyOutcome {
    /// The policy key that produced this outcome.
    pub key: String,
    /// The artifact this outcome talks about, when applicable.
    pub target: Option<QualifiedName>,
    /// Pass / Warn / Fail / WarnSkip.
    pub status: OutcomeStatus,
}

/// Per-outcome status.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OutcomeStatus {
    /// The policy is satisfied.
    Pass,
    /// Soft (advisory) violation.
    Warn {
        /// Human-readable explanation / hint.
        msg: String,
    },
    /// Hard (enforced) violation.
    Fail {
        /// Human-readable explanation / hint.
        msg: String,
    },
    /// The evaluator could not run; `aenv doctor` prints a warning and moves on.
    WarnSkip {
        /// Why the evaluator skipped.
        msg: String,
    },
}

impl PolicyOutcome {
    /// Construct a passing outcome.
    pub fn pass(key: impl Into<String>, target: Option<QualifiedName>) -> Self {
        Self {
            key: key.into(),
            target,
            status: OutcomeStatus::Pass,
        }
    }
    /// Construct a warning (advisory violation).
    pub fn warn(
        key: impl Into<String>,
        target: Option<QualifiedName>,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            target,
            status: OutcomeStatus::Warn { msg: msg.into() },
        }
    }
    /// Construct a hard failure (enforce-policy violation).
    pub fn fail(
        key: impl Into<String>,
        target: Option<QualifiedName>,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            target,
            status: OutcomeStatus::Fail { msg: msg.into() },
        }
    }
    /// Construct a "skipped" outcome (unknown key, evaluator unavailable).
    pub fn warn_skip(key: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            target: None,
            status: OutcomeStatus::WarnSkip { msg: msg.into() },
        }
    }
}

/// The interface implemented by every built-in evaluator.
///
/// `evaluate` returns the outcomes for *this* policy against the namespace
/// in context. Returning an empty Vec is legal (e.g. a policy with no
/// applicable artifacts).
pub trait PolicyEvaluator<F: Filesystem> {
    /// Evaluate the policy and produce a flat list of outcomes.
    fn evaluate(&self, policy: &ResolvedPolicy, ctx: &PolicyContext<F>) -> Vec<PolicyOutcome>;
}

/// Route a resolved policy to its evaluator. Tasks 9–12 add the actual
/// evaluators; for now everything is unknown and produces `WarnSkip`.
pub fn dispatch<F: Filesystem>(
    key: &str,
    _policy: &ResolvedPolicy,
    _ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    // Tasks 9–12 will add `match` arms for the four built-in keys.
    vec![PolicyOutcome::warn_skip(
        key.to_owned(),
        format!("no built-in evaluator for policy key '{key}'"),
    )]
}

#[cfg(any(test, doctest))]
impl<'a> PolicyContext<'a, crate::fs::MockFilesystem> {
    /// Dummy context for tests that don't actually exercise the evaluator —
    /// used by the scaffold test and any future fast-path tests.
    pub fn dummy() -> Self {
        // Note: leaks owned values to give a `'static` borrow. Only call from
        // tests. The leaks are negligible (one Filesystem, one RegistryLayout,
        // one AdapterRegistry, one ResolutionResult per call).
        let fs: &'static crate::fs::MockFilesystem =
            Box::leak(Box::new(crate::fs::MockFilesystem::new()));
        let layout: &'static crate::home::RegistryLayout =
            Box::leak(Box::new(crate::home::RegistryLayout::new(
                std::path::PathBuf::from("/dummy"),
            )));
        let adapters: &'static crate::adapter::AdapterRegistry =
            Box::leak(Box::new(crate::adapter::AdapterRegistry::new()));
        let resolved: &'static ResolutionResult = Box::leak(Box::new(ResolutionResult {
            chain: vec![],
            candidates: vec![],
        }));
        PolicyContext {
            fs,
            layout,
            adapters,
            resolved,
        }
    }
}
```

Add to `crates/aenv-core/src/policies/mod.rs` (the file you renamed):

```rust
pub mod builtin;
```

- [ ] **Step 5: Run the test**

Run: `cargo test -p aenv-core --test policy_evaluator_scaffold 2>&1 | tail -10`
Expected: PASS — 5 tests passed.

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/policies/
git commit -m "Add PolicyEvaluator trait + dispatch scaffold"
```

---

### Task 9: `instructions_max_chars` evaluator

Walks the resolved candidates, identifies the ones whose declaring adapter marks the path as `role = "instructions"`, reads each file's bytes, and emits an outcome per file: `Pass` when `char_count <= limit`, `Warn` (or `Fail` if `enforce = true`) otherwise. UTF-8 char counting (Rust's `str::chars().count()`) is the spec's "characters" — not bytes.

The evaluator skips candidates whose source path does not exist on disk (e.g. when a manifest declares a file but the file is missing — that's a different error class, caught elsewhere).

**Files:**
- Create: `crates/aenv-core/src/policies/builtin/instructions_max_chars.rs`
- Modify: `crates/aenv-core/src/policies/builtin/mod.rs` (wire into `dispatch`)
- Test: `crates/aenv-core/tests/policy_instructions_max_chars.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/policy_instructions_max_chars.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::{dispatch, OutcomeStatus, PolicyContext};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn make_registry_with_claude() -> AdapterRegistry {
    let mut adapters = AdapterRegistry::new();
    let mut roles = BTreeMap::new();
    roles.insert("CLAUDE.md".into(), "instructions".into());
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
    });
    adapters
}

fn make_layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv-home"))
}

fn make_resolution(ns_name: &str, source_path: PathBuf) -> ResolutionResult {
    ResolutionResult {
        chain: vec![ns(ns_name)],
        candidates: vec![Candidate {
            namespace: ns(ns_name),
            path: PathBuf::from("CLAUDE.md"),
            source_path,
            adapter: "claude-code".into(),
            merge_override: None,
        }],
    }
}

#[test]
fn pass_when_under_limit() {
    let fs = MockFilesystem::new();
    fs.write(&PathBuf::from("/aenv-home/envs/base/CLAUDE.md"), b"hello")
        .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = make_resolution("base", PathBuf::from("/aenv-home/envs/base/CLAUDE.md"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };

    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5000),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass), "out = {out:?}");
}

#[test]
fn warn_when_over_limit_and_advisory() {
    let fs = MockFilesystem::new();
    let body = "x".repeat(6000);
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = make_resolution("base", PathBuf::from("/aenv-home/envs/base/CLAUDE.md"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5000),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert_eq!(out.len(), 1);
    if let OutcomeStatus::Warn { msg } = &out[0].status {
        assert!(msg.contains("6000"));
        assert!(msg.contains("5000"));
    } else {
        panic!("expected Warn, got {:?}", out[0].status);
    }
}

#[test]
fn fail_when_over_limit_and_enforced() {
    let fs = MockFilesystem::new();
    let body = "x".repeat(6000);
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = make_resolution("base", PathBuf::from("/aenv-home/envs/base/CLAUDE.md"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5000),
        enforce: true,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Fail { .. }));
}

#[test]
fn counts_utf8_chars_not_bytes() {
    // "é" is 2 bytes but 1 char. Limit = 5 chars; body has 4 chars ("éééé").
    let fs = MockFilesystem::new();
    let body = "éééé"; // 4 chars, 8 bytes
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = make_resolution("base", PathBuf::from("/aenv-home/envs/base/CLAUDE.md"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn skips_non_instructions_files() {
    // Adapter registers CLAUDE.md but candidate path is some other file
    // not declared as `instructions` role.
    let fs = MockFilesystem::new();
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/.mcp.json"),
        b"{ \"servers\": {} }",
    )
    .unwrap();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from(".mcp.json"),
            source_path: PathBuf::from("/aenv-home/envs/base/.mcp.json"),
            adapter: "claude-code".into(),
            merge_override: None,
        }],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Integer(5000),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert!(
        out.is_empty(),
        "expected zero outcomes when no instructions files match; got {out:?}"
    );
}

#[test]
fn wrong_value_type_warn_skips() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = make_registry_with_claude();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("instructions_max_chars", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::WarnSkip { .. }));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test policy_instructions_max_chars 2>&1 | tail -10`
Expected: FAIL — the dispatch table doesn't know `instructions_max_chars` yet (returns `WarnSkip` for everything).

- [ ] **Step 3: Implement the evaluator**

Create `crates/aenv-core/src/policies/builtin/instructions_max_chars.rs`:

```rust
//! `instructions_max_chars`: cap on the UTF-8 character count of any
//! adapter-managed file with `role = "instructions"`.

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::policies::builtin::{OutcomeStatus, PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "instructions_max_chars";

/// Evaluate the policy against every `role = "instructions"` candidate.
pub fn evaluate<F: Filesystem>(
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    let limit = match &policy.value {
        PolicyValue::Integer(n) if *n >= 0 => *n as usize,
        _ => {
            return vec![PolicyOutcome::warn_skip(
                KEY,
                format!(
                    "policy '{KEY}' must be a non-negative integer; got {} (source: {})",
                    policy.value.type_tag(),
                    policy.source
                ),
            )];
        }
    };

    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for c in &ctx.resolved.candidates {
        let adapter = match ctx.adapters.get(&c.adapter) {
            Some(a) => a,
            None => continue,
        };
        let role = adapter
            .roles
            .get(c.path.to_string_lossy().as_ref())
            .map(String::as_str)
            .unwrap_or("");
        if role != "instructions" {
            continue;
        }
        let target = QualifiedName::new(
            c.namespace.clone(),
            ShortName::new(c.path.to_string_lossy().to_string()).unwrap_or_else(|_| {
                ShortName::new("?".to_string()).expect("trivial short name is valid")
            }),
        );

        let bytes = match ctx.fs.read(&c.source_path) {
            Ok(b) => b,
            Err(e) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!(
                        "cannot read instructions file {}: {e}",
                        c.source_path.display()
                    ),
                ));
                continue;
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!(
                        "instructions file {} is not valid UTF-8; cannot count chars",
                        c.source_path.display()
                    ),
                ));
                continue;
            }
        };
        let chars = text.chars().count();
        if chars <= limit {
            outcomes.push(PolicyOutcome::pass(KEY, Some(target)));
        } else {
            let msg = format!(
                "{} has {chars} chars (budget {limit}). Refactor procedural content into \
                 skills, dispositional content into subagents, or use @-imports.",
                c.path.display()
            );
            outcomes.push(if policy.enforce {
                PolicyOutcome::fail(KEY, Some(target), msg)
            } else {
                PolicyOutcome::warn(KEY, Some(target), msg)
            });
        }
    }
    outcomes
}
```

Now wire it into `dispatch`. Modify `crates/aenv-core/src/policies/builtin/mod.rs` — replace the existing `dispatch` body:

```rust
pub mod instructions_max_chars;

pub fn dispatch<F: Filesystem>(
    key: &str,
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    match key {
        "instructions_max_chars" => instructions_max_chars::evaluate(policy, ctx),
        other => vec![PolicyOutcome::warn_skip(
            other.to_owned(),
            format!("no built-in evaluator for policy key '{other}'"),
        )],
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test policy_instructions_max_chars 2>&1 | tail -10`
Expected: PASS — 6 tests passed.

Run: `cargo test -p aenv-core --test policy_evaluator_scaffold 2>&1 | tail -10`
Expected: still PASS — the unknown-key path still WarnSkips.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/policies/builtin/instructions_max_chars.rs crates/aenv-core/src/policies/builtin/mod.rs crates/aenv-core/tests/policy_instructions_max_chars.rs
git commit -m "Add instructions_max_chars policy evaluator"
```

---

### Task 10: `skill_requires_description` evaluator

Walks the resolved candidates, identifies the ones that look like skill files (parent dir is `.claude/skills/<name>/` and filename is `SKILL.md`), and verifies that the YAML frontmatter declares a non-empty `description:` field. Missing frontmatter, missing field, or empty value all fail.

This is the only Phase 3 policy that has to crack a file format: SKILL.md uses Jekyll-style YAML frontmatter bracketed by `---` lines. We use a tiny line-based parser — no `serde_yaml` for this. Frontmatter must start on line 1.

**Files:**
- Create: `crates/aenv-core/src/policies/builtin/skill_requires_description.rs`
- Modify: `crates/aenv-core/src/policies/builtin/mod.rs` (wire into `dispatch`)
- Test: `crates/aenv-core/tests/policy_skill_requires_description.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/policy_skill_requires_description.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::{dispatch, OutcomeStatus, PolicyContext};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn make_registry() -> AdapterRegistry {
    let mut adapters = AdapterRegistry::new();
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec![".claude/".into()],
        merge_strategies: BTreeMap::new(),
        roles: BTreeMap::new(),
        default_merge: BTreeMap::new(),
        parameters: vec![],
    });
    adapters
}

fn make_layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv-home"))
}

fn skill_candidate(ns_name: &str, skill: &str, source: PathBuf) -> Candidate {
    Candidate {
        namespace: ns(ns_name),
        path: PathBuf::from(format!(".claude/skills/{skill}/SKILL.md")),
        source_path: source,
        adapter: "claude-code".into(),
        merge_override: None,
    }
}

#[test]
fn pass_when_description_present() {
    let fs = MockFilesystem::new();
    let body = "---\nname: write-tests\ndescription: Writes tests for changed code\n---\nBody";
    fs.write(
        &PathBuf::from("/aenv-home/envs/base/.claude/skills/write-tests/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![skill_candidate(
            "base",
            "write-tests",
            PathBuf::from("/aenv-home/envs/base/.claude/skills/write-tests/SKILL.md"),
        )],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn warn_when_description_missing() {
    let fs = MockFilesystem::new();
    let body = "---\nname: half-baked\n---\nBody";
    fs.write(
        &PathBuf::from("/aenv-home/envs/x/.claude/skills/half-baked/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("x")],
        candidates: vec![skill_candidate(
            "x",
            "half-baked",
            PathBuf::from("/aenv-home/envs/x/.claude/skills/half-baked/SKILL.md"),
        )],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    if let OutcomeStatus::Warn { msg } = &out[0].status {
        assert!(msg.contains("description"));
        assert!(msg.contains("half-baked"));
    } else {
        panic!("expected Warn, got {:?}", out[0].status);
    }
}

#[test]
fn fail_when_enforced_and_description_missing() {
    let fs = MockFilesystem::new();
    let body = "---\nname: half-baked\n---\nBody";
    fs.write(
        &PathBuf::from("/aenv-home/envs/x/.claude/skills/half-baked/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("x")],
        candidates: vec![skill_candidate(
            "x",
            "half-baked",
            PathBuf::from("/aenv-home/envs/x/.claude/skills/half-baked/SKILL.md"),
        )],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: true,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Fail { .. }));
}

#[test]
fn fail_when_description_empty() {
    let fs = MockFilesystem::new();
    let body = "---\nname: x\ndescription:   \n---\nBody";
    fs.write(
        &PathBuf::from("/aenv-home/envs/x/.claude/skills/x/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("x")],
        candidates: vec![skill_candidate(
            "x",
            "x",
            PathBuf::from("/aenv-home/envs/x/.claude/skills/x/SKILL.md"),
        )],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Warn { .. }));
}

#[test]
fn warn_when_no_frontmatter_at_all() {
    let fs = MockFilesystem::new();
    let body = "no frontmatter here";
    fs.write(
        &PathBuf::from("/aenv-home/envs/x/.claude/skills/raw/SKILL.md"),
        body.as_bytes(),
    )
    .unwrap();
    let adapters = make_registry();
    let layout = make_layout();
    let resolved = ResolutionResult {
        chain: vec![ns("x")],
        candidates: vec![skill_candidate(
            "x",
            "raw",
            PathBuf::from("/aenv-home/envs/x/.claude/skills/raw/SKILL.md"),
        )],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Warn { .. }));
}

#[test]
fn skips_non_skill_files() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = make_registry();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/aenv-home/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
        }],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    assert!(out.is_empty());
}

#[test]
fn disabled_when_false() {
    let fs = MockFilesystem::new();
    // No body even needed — the policy is off.
    let layout = make_layout();
    let adapters = make_registry();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![skill_candidate(
            "base",
            "x",
            PathBuf::from("/aenv-home/envs/base/.claude/skills/x/SKILL.md"),
        )],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(false),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("skill_requires_description", &rp, &ctx);
    // When value is false the policy is off; no outcomes.
    assert!(out.is_empty());
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test policy_skill_requires_description 2>&1 | tail -10`
Expected: FAIL — dispatch still WarnSkips this key.

- [ ] **Step 3: Implement the evaluator**

Create `crates/aenv-core/src/policies/builtin/skill_requires_description.rs`:

```rust
//! `skill_requires_description`: every authored skill file's YAML frontmatter
//! must declare a non-empty `description:` field.

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::policies::builtin::{PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "skill_requires_description";

/// Evaluate the policy against every resolved candidate that looks like a
/// skill file: relative path matches `.claude/skills/<dir>/SKILL.md`.
pub fn evaluate<F: Filesystem>(
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    let active = match &policy.value {
        PolicyValue::Boolean(b) => *b,
        _ => {
            return vec![PolicyOutcome::warn_skip(
                KEY,
                format!(
                    "policy '{KEY}' must be a boolean; got {} (source: {})",
                    policy.value.type_tag(),
                    policy.source
                ),
            )];
        }
    };
    if !active {
        return Vec::new();
    }

    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for c in &ctx.resolved.candidates {
        if !looks_like_skill_file(&c.path) {
            continue;
        }
        let target = QualifiedName::new(
            c.namespace.clone(),
            ShortName::new(c.path.to_string_lossy().to_string()).unwrap_or_else(|_| {
                ShortName::new("?".to_string()).expect("trivial short name is valid")
            }),
        );
        let bytes = match ctx.fs.read(&c.source_path) {
            Ok(b) => b,
            Err(e) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!("cannot read {}: {e}", c.source_path.display()),
                ));
                continue;
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!("{} is not valid UTF-8", c.source_path.display()),
                ));
                continue;
            }
        };
        match extract_description(text) {
            DescriptionResult::Present => outcomes.push(PolicyOutcome::pass(KEY, Some(target))),
            other => {
                let msg = match other {
                    DescriptionResult::NoFrontmatter => format!(
                        "{}: no YAML frontmatter found. Add a '---' block at the top with 'name:' and 'description:'.",
                        c.path.display()
                    ),
                    DescriptionResult::Missing => format!(
                        "{}: frontmatter is missing 'description:'. The description tells the model when to invoke the skill.",
                        c.path.display()
                    ),
                    DescriptionResult::Empty => format!(
                        "{}: 'description:' field is empty. Add a one-sentence description so the model knows when to invoke this skill.",
                        c.path.display()
                    ),
                    DescriptionResult::Present => unreachable!(),
                };
                outcomes.push(if policy.enforce {
                    PolicyOutcome::fail(KEY, Some(target), msg)
                } else {
                    PolicyOutcome::warn(KEY, Some(target), msg)
                });
            }
        }
    }
    outcomes
}

fn looks_like_skill_file(rel: &std::path::Path) -> bool {
    let s = rel.to_string_lossy();
    // Match `.claude/skills/<one component>/SKILL.md` exactly.
    let parts: Vec<&str> = s.split('/').collect();
    parts.len() == 4
        && parts[0] == ".claude"
        && parts[1] == "skills"
        && !parts[2].is_empty()
        && parts[3] == "SKILL.md"
}

#[derive(Debug)]
enum DescriptionResult {
    NoFrontmatter,
    Missing,
    Empty,
    Present,
}

fn extract_description(text: &str) -> DescriptionResult {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return DescriptionResult::NoFrontmatter;
    }
    let mut found: Option<String> = None;
    for line in lines.by_ref() {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("description:") {
            found = Some(rest.trim().to_string());
        }
    }
    match found {
        Some(v) if v.is_empty() => DescriptionResult::Empty,
        Some(_) => DescriptionResult::Present,
        None => DescriptionResult::Missing,
    }
}
```

Wire into `dispatch` (modify `policies/builtin/mod.rs`):

```rust
pub mod skill_requires_description;

// inside dispatch:
match key {
    "instructions_max_chars" => instructions_max_chars::evaluate(policy, ctx),
    "skill_requires_description" => skill_requires_description::evaluate(policy, ctx),
    other => vec![PolicyOutcome::warn_skip(
        other.to_owned(),
        format!("no built-in evaluator for policy key '{other}'"),
    )],
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test policy_skill_requires_description 2>&1 | tail -10`
Expected: PASS — 7 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/policies/builtin/skill_requires_description.rs crates/aenv-core/src/policies/builtin/mod.rs crates/aenv-core/tests/policy_skill_requires_description.rs
git commit -m "Add skill_requires_description policy evaluator"
```

---

### Task 11: `mcp_requires_command_or_url` evaluator

Walks resolved candidates with path `.mcp.json` (or any path the adapter declares with `role = "mcp"`), parses the JSON, and checks that every entry under the top-level `mcpServers` (or `servers`) object declares either `command` or `url`. Missing both is a violation.

This evaluator runs against *single-namespace* candidates only — for merged `.mcp.json`s, the merge happens at activation time and the merged file's outcomes will be reported under the contributors' identities once Phase 3 wires the activation-time evaluator (Task 13). For the doctor walk before activation, we evaluate each contributor independently — which is the right behavior: a problem in any contributor is a problem.

**Files:**
- Create: `crates/aenv-core/src/policies/builtin/mcp_requires_command_or_url.rs`
- Modify: `crates/aenv-core/src/policies/builtin/mod.rs` (wire into `dispatch`)
- Test: `crates/aenv-core/tests/policy_mcp_requires_command_or_url.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/policy_mcp_requires_command_or_url.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::{dispatch, OutcomeStatus, PolicyContext};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn make_adapters() -> AdapterRegistry {
    let mut adapters = AdapterRegistry::new();
    let mut roles = BTreeMap::new();
    roles.insert(".mcp.json".into(), "mcp".into());
    adapters.insert(Adapter {
        name: "mcp".into(),
        files: vec![".mcp.json".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
    });
    adapters
}

fn make_resolved(ns_name: &str, source: PathBuf) -> ResolutionResult {
    ResolutionResult {
        chain: vec![ns(ns_name)],
        candidates: vec![Candidate {
            namespace: ns(ns_name),
            path: PathBuf::from(".mcp.json"),
            source_path: source,
            adapter: "mcp".into(),
            merge_override: None,
        }],
    }
}

fn enforce_true() -> ResolvedPolicy {
    ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: true,
        source: ns("base"),
    }
}

fn advisory() -> ResolvedPolicy {
    ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    }
}

#[test]
fn pass_when_command_present() {
    let fs = MockFilesystem::new();
    let body = br#"{"mcpServers":{"fs":{"command":"npx fs-mcp"}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn pass_when_url_present() {
    let fs = MockFilesystem::new();
    let body = br#"{"mcpServers":{"net":{"url":"https://example/mcp"}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn warn_when_neither_advisory() {
    let fs = MockFilesystem::new();
    let body = br#"{"mcpServers":{"broken":{"timeout":30}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    if let OutcomeStatus::Warn { msg } = &out[0].status {
        assert!(msg.contains("broken"));
        assert!(msg.contains("command") || msg.contains("url"));
    } else {
        panic!("expected Warn, got {:?}", out[0].status);
    }
}

#[test]
fn fail_when_neither_enforced() {
    let fs = MockFilesystem::new();
    let body = br#"{"mcpServers":{"broken":{"timeout":30}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &enforce_true(), &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Fail { .. }));
}

#[test]
fn warn_skip_when_json_invalid() {
    let fs = MockFilesystem::new();
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), b"not json")
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::WarnSkip { .. }));
}

#[test]
fn skips_non_mcp_files() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles: BTreeMap::new(),
        default_merge: BTreeMap::new(),
        parameters: vec![],
    });
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
        }],
    };
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert!(out.is_empty());
}

#[test]
fn accepts_servers_root_alias() {
    // Some configs use the bare `servers` key instead of `mcpServers`.
    let fs = MockFilesystem::new();
    let body = br#"{"servers":{"fs":{"command":"npx fs-mcp"}}}"#;
    fs.write(&PathBuf::from("/h/envs/base/.mcp.json"), body)
        .unwrap();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let out = dispatch("mcp_requires_command_or_url", &advisory(), &ctx);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn disabled_when_false() {
    let fs = MockFilesystem::new();
    let resolved = make_resolved("base", PathBuf::from("/h/envs/base/.mcp.json"));
    let adapters = make_adapters();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let ctx = PolicyContext {
        fs: &fs,
        layout: &layout,
        adapters: &adapters,
        resolved: &resolved,
    };
    let rp = ResolvedPolicy {
        value: PolicyValue::Boolean(false),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("mcp_requires_command_or_url", &rp, &ctx);
    assert!(out.is_empty());
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test policy_mcp_requires_command_or_url 2>&1 | tail -10`
Expected: FAIL — dispatch still WarnSkips this key.

- [ ] **Step 3: Implement the evaluator**

Create `crates/aenv-core/src/policies/builtin/mcp_requires_command_or_url.rs`:

```rust
//! `mcp_requires_command_or_url`: every entry under `mcpServers` (or `servers`)
//! in an MCP-role JSON file must declare `command` or `url`.

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::policies::builtin::{PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "mcp_requires_command_or_url";

/// Evaluate the policy. Looks for the `mcpServers` key, or `servers` as
/// alias. Per-server outcome is rolled up into a single Pass for the file
/// (if every server is fine) or one Warn/Fail per offending server.
pub fn evaluate<F: Filesystem>(
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    let active = match &policy.value {
        PolicyValue::Boolean(b) => *b,
        _ => {
            return vec![PolicyOutcome::warn_skip(
                KEY,
                format!(
                    "policy '{KEY}' must be a boolean; got {} (source: {})",
                    policy.value.type_tag(),
                    policy.source
                ),
            )];
        }
    };
    if !active {
        return Vec::new();
    }

    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for c in &ctx.resolved.candidates {
        let adapter = match ctx.adapters.get(&c.adapter) {
            Some(a) => a,
            None => continue,
        };
        let role = adapter
            .roles
            .get(c.path.to_string_lossy().as_ref())
            .map(String::as_str)
            .unwrap_or("");
        if role != "mcp" {
            continue;
        }
        let target = QualifiedName::new(
            c.namespace.clone(),
            ShortName::new(c.path.to_string_lossy().to_string()).unwrap_or_else(|_| {
                ShortName::new("?".to_string()).expect("trivial short name is valid")
            }),
        );
        let bytes = match ctx.fs.read(&c.source_path) {
            Ok(b) => b,
            Err(e) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!("cannot read {}: {e}", c.source_path.display()),
                ));
                continue;
            }
        };
        let v: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                outcomes.push(PolicyOutcome::warn_skip(
                    KEY,
                    format!("{} is not valid JSON: {e}", c.source_path.display()),
                ));
                continue;
            }
        };
        let servers = v
            .get("mcpServers")
            .or_else(|| v.get("servers"))
            .and_then(|x| x.as_object());
        let servers = match servers {
            Some(s) => s,
            None => {
                outcomes.push(PolicyOutcome::pass(KEY, Some(target.clone())));
                continue;
            }
        };
        let mut violations: Vec<String> = Vec::new();
        for (name, body) in servers {
            let ok = body
                .as_object()
                .map(|o| o.contains_key("command") || o.contains_key("url"))
                .unwrap_or(false);
            if !ok {
                violations.push(name.clone());
            }
        }
        if violations.is_empty() {
            outcomes.push(PolicyOutcome::pass(KEY, Some(target)));
        } else {
            let msg = format!(
                "{}: server(s) [{}] declare neither 'command' nor 'url'. \
                 Add one so the server can be reached.",
                c.path.display(),
                violations.join(", ")
            );
            outcomes.push(if policy.enforce {
                PolicyOutcome::fail(KEY, Some(target), msg)
            } else {
                PolicyOutcome::warn(KEY, Some(target), msg)
            });
        }
    }
    outcomes
}
```

Wire into `dispatch` (modify `policies/builtin/mod.rs`):

```rust
pub mod mcp_requires_command_or_url;

// add an arm to the match:
"mcp_requires_command_or_url" => mcp_requires_command_or_url::evaluate(policy, ctx),
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test policy_mcp_requires_command_or_url 2>&1 | tail -10`
Expected: PASS — 8 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/policies/builtin/mcp_requires_command_or_url.rs crates/aenv-core/src/policies/builtin/mod.rs crates/aenv-core/tests/policy_mcp_requires_command_or_url.rs
git commit -m "Add mcp_requires_command_or_url policy evaluator"
```

---

### Task 12: `forbid_paths` evaluator

Walks resolved candidates and emits a Warn/Fail per artifact whose materialized path matches any glob in the deny-list. Uses the same minimal glob matcher Phase 2 uses elsewhere (literal match plus `**/*` suffix and a simple `*` prefix). When no candidates match, the evaluator emits a single `Pass` outcome with no target — confirms the policy ran without surfacing per-file noise.

**Files:**
- Create: `crates/aenv-core/src/policies/builtin/forbid_paths.rs`
- Modify: `crates/aenv-core/src/policies/builtin/mod.rs` (wire into `dispatch`)
- Test: `crates/aenv-core/tests/policy_forbid_paths.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/policy_forbid_paths.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::MockFilesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::{dispatch, OutcomeStatus, PolicyContext};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn dummy_adapter() -> Adapter {
    Adapter {
        name: "claude-code".into(),
        files: vec![],
        merge_strategies: BTreeMap::new(),
        roles: BTreeMap::new(),
        default_merge: BTreeMap::new(),
        parameters: vec![],
    }
}

fn candidate(rel: &str) -> Candidate {
    Candidate {
        namespace: ns("base"),
        path: PathBuf::from(rel),
        source_path: PathBuf::from(format!("/h/envs/base/{rel}")),
        adapter: "claude-code".into(),
        merge_override: None,
    }
}

fn forbid(value: Vec<&str>, enforce: bool) -> ResolvedPolicy {
    ResolvedPolicy {
        value: PolicyValue::ListString(value.into_iter().map(String::from).collect()),
        enforce,
        source: ns("base"),
    }
}

fn ctx<'a>(
    fs: &'a MockFilesystem,
    layout: &'a RegistryLayout,
    adapters: &'a AdapterRegistry,
    resolved: &'a ResolutionResult,
) -> PolicyContext<'a, MockFilesystem> {
    PolicyContext {
        fs,
        layout,
        adapters,
        resolved,
    }
}

#[test]
fn exact_match_advisory_warns() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate(".env")],
    };
    let policy = forbid(vec![".env"], false);
    let out = dispatch("forbid_paths", &policy, &ctx(&fs, &layout, &adapters, &resolved));
    assert_eq!(out.len(), 1);
    if let OutcomeStatus::Warn { msg } = &out[0].status {
        assert!(msg.contains(".env"));
    } else {
        panic!("expected Warn, got {:?}", out[0].status);
    }
}

#[test]
fn exact_match_enforced_fails() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate(".env")],
    };
    let policy = forbid(vec![".env"], true);
    let out = dispatch("forbid_paths", &policy, &ctx(&fs, &layout, &adapters, &resolved));
    assert!(matches!(out[0].status, OutcomeStatus::Fail { .. }));
}

#[test]
fn star_suffix_matches() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate(".env.production")],
    };
    let policy = forbid(vec![".env*"], false);
    let out = dispatch("forbid_paths", &policy, &ctx(&fs, &layout, &adapters, &resolved));
    assert!(matches!(out[0].status, OutcomeStatus::Warn { .. }));
}

#[test]
fn glob_double_star_matches_subtree() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate("secrets/db.json")],
    };
    let policy = forbid(vec!["secrets/**"], false);
    let out = dispatch("forbid_paths", &policy, &ctx(&fs, &layout, &adapters, &resolved));
    assert!(matches!(out[0].status, OutcomeStatus::Warn { .. }));
}

#[test]
fn pass_outcome_when_no_match() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate("CLAUDE.md")],
    };
    let policy = forbid(vec![".env*", "secrets/**"], false);
    let out = dispatch("forbid_paths", &policy, &ctx(&fs, &layout, &adapters, &resolved));
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn empty_list_passes() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let mut adapters = AdapterRegistry::new();
    adapters.insert(dummy_adapter());
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![candidate(".env")],
    };
    let policy = forbid(vec![], false);
    let out = dispatch("forbid_paths", &policy, &ctx(&fs, &layout, &adapters, &resolved));
    assert!(matches!(out[0].status, OutcomeStatus::Pass));
}

#[test]
fn wrong_value_type_warn_skips() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let adapters = AdapterRegistry::new();
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![],
    };
    let policy = ResolvedPolicy {
        value: PolicyValue::Boolean(true),
        enforce: false,
        source: ns("base"),
    };
    let out = dispatch("forbid_paths", &policy, &ctx(&fs, &layout, &adapters, &resolved));
    assert!(matches!(out[0].status, OutcomeStatus::WarnSkip { .. }));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test policy_forbid_paths 2>&1 | tail -10`
Expected: FAIL — dispatch still WarnSkips this key.

- [ ] **Step 3: Implement the evaluator**

Create `crates/aenv-core/src/policies/builtin/forbid_paths.rs`:

```rust
//! `forbid_paths`: deny-list of materialized paths.

use crate::fs::Filesystem;
use crate::identity::{QualifiedName, ShortName};
use crate::policies::builtin::{PolicyContext, PolicyOutcome};
use crate::policies::{PolicyValue, ResolvedPolicy};

const KEY: &str = "forbid_paths";

/// Evaluate every resolved candidate against the patterns. Emits a single
/// `Pass` outcome with no target if nothing matched; emits a Warn/Fail per
/// matching candidate otherwise.
pub fn evaluate<F: Filesystem>(
    policy: &ResolvedPolicy,
    ctx: &PolicyContext<F>,
) -> Vec<PolicyOutcome> {
    let patterns: &Vec<String> = match &policy.value {
        PolicyValue::ListString(xs) => xs,
        _ => {
            return vec![PolicyOutcome::warn_skip(
                KEY,
                format!(
                    "policy '{KEY}' must be a list-of-string; got {} (source: {})",
                    policy.value.type_tag(),
                    policy.source
                ),
            )];
        }
    };

    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for c in &ctx.resolved.candidates {
        let rel = c.path.to_string_lossy().to_string();
        let hit = patterns.iter().any(|p| forbid_match(p, &rel));
        if !hit {
            continue;
        }
        let target = QualifiedName::new(
            c.namespace.clone(),
            ShortName::new(rel.clone()).unwrap_or_else(|_| ShortName::new("?".into()).unwrap()),
        );
        let msg = format!(
            "{} matches forbid_paths pattern; namespace must not declare this path.",
            rel
        );
        outcomes.push(if policy.enforce {
            PolicyOutcome::fail(KEY, Some(target), msg)
        } else {
            PolicyOutcome::warn(KEY, Some(target), msg)
        });
    }
    if outcomes.is_empty() {
        outcomes.push(PolicyOutcome::pass(KEY, None));
    }
    outcomes
}

fn forbid_match(pattern: &str, candidate: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        candidate.starts_with(prefix) && candidate[prefix.len()..].starts_with('/')
    } else if let Some(prefix) = pattern.strip_suffix("/**/*") {
        candidate.starts_with(prefix) && candidate[prefix.len()..].starts_with('/')
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        candidate.starts_with(prefix)
    } else {
        pattern == candidate
    }
}
```

Wire into `dispatch` (modify `policies/builtin/mod.rs`):

```rust
pub mod forbid_paths;

// add to match:
"forbid_paths" => forbid_paths::evaluate(policy, ctx),
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test policy_forbid_paths 2>&1 | tail -10`
Expected: PASS — 7 tests passed.

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/policies/builtin/forbid_paths.rs crates/aenv-core/src/policies/builtin/mod.rs crates/aenv-core/tests/policy_forbid_paths.rs
git commit -m "Add forbid_paths policy evaluator"
```

---

### Task 13: Wire parameters + policies into `ResolutionResult` and state.json (schema 3)

The pieces exist; this task plumbs them together. After this task, calling `resolve_namespace` returns the chain, candidates, *and* the resolved parameter + policy maps. The activation state schema bumps to 3 to record both maps; schema 2 still reads (defaults both to empty maps).

**Files:**
- Modify: `crates/aenv-core/src/resolve.rs` — `ResolutionResult` gains `parameters`, `policies`; `resolve_namespace` populates them
- Modify: `crates/aenv-core/src/state.rs` — bump `SCHEMA_VERSION` to `3`; `ActivationState` gains `parameters`, `policies` fields; schema-2 reader path
- Modify: `crates/aenv-core/src/activate/mod.rs` — write resolved params + policies into the `ActivationState`
- Test: `crates/aenv-core/tests/resolution_with_params_and_policies.rs`
- Test: `crates/aenv-core/tests/state_schema.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/aenv-core/tests/resolution_with_params_and_policies.rs`:

```rust
use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::parameters::ParameterValue;
use aenv_core::policies::PolicyValue;
use aenv_core::resolve::resolve_namespace;
use std::path::PathBuf;

fn write_manifest(fs: &MockFilesystem, layout: &RegistryLayout, name: &str, body: &str) {
    fs.write(&layout.manifest_path(name), body.as_bytes()).unwrap();
    fs.write(
        &layout.namespace_dir(name).join("CLAUDE.md"),
        b"placeholder",
    )
    .unwrap();
}

#[test]
fn resolves_parameters_and_policies_from_chain() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let adapters = AdapterRegistry::new(); // empty adapters → ignore the [adapters.x] field; but resolver still walks
    // Even with empty adapter registry, the manifests below declare no adapters,
    // so resolution should succeed.

    write_manifest(
        &fs,
        &layout,
        "base",
        r#"
name = "base"

[parameters]
default_model = "haiku"
budget = 5000

[policies]
skill_requires_description = true
"#,
    );
    write_manifest(
        &fs,
        &layout,
        "leaf",
        r#"
name = "leaf"
extends = ["base"]

[parameters]
default_model = "opus"

[policies]
instructions_max_chars = { value = 3000, enforce = true }
"#,
    );

    let r = resolve_namespace(&fs, &layout, &adapters, &NamespaceId::new("leaf").unwrap()).unwrap();

    let params = r.parameters;
    let policies = r.policies;
    assert_eq!(
        params.get("default_model").unwrap().value,
        ParameterValue::String("opus".into())
    );
    assert_eq!(params.get("default_model").unwrap().source.as_str(), "leaf");
    assert_eq!(
        params.get("budget").unwrap().value,
        ParameterValue::Integer(5000)
    );
    assert_eq!(params.get("budget").unwrap().source.as_str(), "base");

    let s = policies.get("skill_requires_description").unwrap();
    assert_eq!(s.value, PolicyValue::Boolean(true));
    assert_eq!(s.source.as_str(), "base");
    let im = policies.get("instructions_max_chars").unwrap();
    assert_eq!(im.value, PolicyValue::Integer(3000));
    assert!(im.enforce);
}
```

Create `crates/aenv-core/tests/state_schema.rs`:

```rust
use aenv_core::identity::{NamespaceId, QualifiedName, ShortName};
use aenv_core::parameters::{ParameterValue, ResolvedParameter};
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::MaterializeStrategy;
use aenv_core::state::{ActivationState, ManagedFile, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn schema_version_is_3() {
    assert_eq!(SCHEMA_VERSION, 3);
}

#[test]
fn schema_3_roundtrip_with_params_and_policies() {
    let qn = QualifiedName::new(
        NamespaceId::new("base").unwrap(),
        ShortName::new("CLAUDE.md").unwrap(),
    );
    let mut parameters = BTreeMap::new();
    parameters.insert(
        "default_model".into(),
        ResolvedParameter {
            value: ParameterValue::String("opus".into()),
            source: NamespaceId::new("leaf").unwrap(),
        },
    );
    let mut policies = BTreeMap::new();
    policies.insert(
        "instructions_max_chars".into(),
        ResolvedPolicy {
            value: PolicyValue::Integer(3000),
            enforce: true,
            source: NamespaceId::new("leaf").unwrap(),
        },
    );
    let state = ActivationState {
        schema_version: 3,
        active_namespace: "leaf".into(),
        project_root: PathBuf::from("/p"),
        managed_files: vec![ManagedFile {
            path: PathBuf::from("CLAUDE.md"),
            qualified_name: qn,
            strategy: MaterializeStrategy::Symlink,
            contributors: vec![],
            shadows: vec![],
        }],
        backed_up: vec![],
        parameters,
        policies,
    };
    let s = state.to_json().unwrap();
    let parsed = ActivationState::from_json(&s).unwrap();
    assert_eq!(parsed, state);
}

#[test]
fn reads_schema_2_with_default_empty_maps() {
    // A schema-2 state file: no parameters, no policies fields.
    let json = r#"{
        "schema_version": 2,
        "active_namespace": "base",
        "project_root": "/p",
        "managed_files": [],
        "backed_up": []
    }"#;
    let s = ActivationState::from_json(json).unwrap();
    assert_eq!(s.schema_version, 2);
    assert!(s.parameters.is_empty());
    assert!(s.policies.is_empty());
}

#[test]
fn rejects_unknown_higher_schema_version() {
    let json = r#"{
        "schema_version": 4,
        "active_namespace": "base",
        "project_root": "/p",
        "managed_files": [],
        "backed_up": []
    }"#;
    let err = ActivationState::from_json(json).unwrap_err();
    assert!(err.to_string().contains("schema_version 4"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test resolution_with_params_and_policies --test state_schema 2>&1 | tail -10`
Expected: FAIL — `ResolutionResult.parameters`, `ActivationState.parameters`, `SCHEMA_VERSION == 3` all missing.

- [ ] **Step 3: Extend `ResolutionResult`**

In `crates/aenv-core/src/resolve.rs`, modify `ResolutionResult`:

```rust
use crate::parameters::{resolve_parameters, ResolvedParameter};
use crate::policies::{resolve_policies, ResolvedPolicy};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolutionResult {
    pub chain: Vec<NamespaceId>,
    pub candidates: Vec<Candidate>,
    pub parameters: BTreeMap<String, ResolvedParameter>,
    pub policies: BTreeMap<String, ResolvedPolicy>,
}
```

Update `resolve_namespace` to populate these after the existing chain/candidate code (just before the final `Ok(...)`):

```rust
let mut params_per_ns: BTreeMap<NamespaceId, BTreeMap<String, crate::parameters::ParameterValue>> = BTreeMap::new();
let mut policies_per_ns: BTreeMap<NamespaceId, BTreeMap<String, crate::policies::PolicyDecl>> = BTreeMap::new();
for ns in &chain {
    let manifest = load_manifest(fs, registry, ns)?;
    params_per_ns.insert(ns.clone(), manifest.parameters.clone());
    policies_per_ns.insert(ns.clone(), manifest.policies.clone());
}
let parameters = resolve_parameters(&chain, &params_per_ns)
    .map_err(|e| ResolutionError::ManifestInvalid {
        namespace: leaf.clone(),
        reason: e.to_string(),
    })?;
let policies = resolve_policies(&chain, &policies_per_ns)
    .map_err(|e| ResolutionError::ManifestInvalid {
        namespace: leaf.clone(),
        reason: e.to_string(),
    })?;
crate::parameters::check_against_adapters(&parameters, adapters)
    .map_err(|e| ResolutionError::ManifestInvalid {
        namespace: leaf.clone(),
        reason: e.to_string(),
    })?;
Ok(ResolutionResult {
    chain,
    candidates,
    parameters,
    policies,
})
```

(Replace the existing trailing `Ok(ResolutionResult { chain, candidates })` with the block above.)

Also update the `policies/builtin/mod.rs` `dummy()` helper to use the new constructor: replace the `ResolutionResult { chain: vec![], candidates: vec![] }` literal with `ResolutionResult { chain: vec![], candidates: vec![], parameters: BTreeMap::new(), policies: BTreeMap::new() }`. Add `use std::collections::BTreeMap;` if not already there.

Phase 2's existing tests pass `ResolutionResult { chain: ..., candidates: ... }` as a struct literal in several integration tests. Find them and update:

```bash
grep -rn "ResolutionResult {" crates/aenv-core/tests/ crates/aenv-core/src/
```

For each hit, add the two new fields (default to `BTreeMap::new()`). The Phase 2 e2e tests already use `resolve_namespace` so they will pick up the extension transparently — only direct struct-literal construction sites need editing.

- [ ] **Step 4: Extend `ActivationState`**

In `crates/aenv-core/src/state.rs`, bump the constant and add fields. Replace `SCHEMA_VERSION` constant and `ActivationState` struct + its custom `Deserialize`:

```rust
pub const SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ActivationState {
    pub schema_version: u32,
    pub active_namespace: String,
    pub project_root: PathBuf,
    pub managed_files: Vec<ManagedFile>,
    pub backed_up: Vec<BackedUpFile>,
    /// Effective parameters after `extends` resolution (Phase 3).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub parameters: std::collections::BTreeMap<String, crate::parameters::ResolvedParameter>,
    /// Effective policies after `extends` resolution (Phase 3).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub policies: std::collections::BTreeMap<String, crate::policies::ResolvedPolicy>,
}

impl<'de> serde::Deserialize<'de> for ActivationState {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        use std::collections::BTreeMap;
        #[derive(serde::Deserialize)]
        struct Raw {
            schema_version: u32,
            active_namespace: String,
            project_root: PathBuf,
            #[serde(default)]
            managed_files: Vec<ManagedFile>,
            #[serde(default)]
            backed_up: Vec<BackedUpFile>,
            #[serde(default)]
            parameters: BTreeMap<String, crate::parameters::ResolvedParameter>,
            #[serde(default)]
            policies: BTreeMap<String, crate::policies::ResolvedPolicy>,
        }
        let mut raw = Raw::deserialize(d)?;
        if raw.schema_version == 1 {
            let ns = crate::identity::NamespaceId::new(raw.active_namespace.as_str())
                .map_err(serde::de::Error::custom)?;
            for mf in &mut raw.managed_files {
                if mf.qualified_name.namespace().as_str() == "__schema_1__" {
                    mf.qualified_name = crate::identity::QualifiedName::new(
                        ns.clone(),
                        mf.qualified_name.short().clone(),
                    );
                }
            }
        }
        Ok(ActivationState {
            schema_version: raw.schema_version,
            active_namespace: raw.active_namespace,
            project_root: raw.project_root,
            managed_files: raw.managed_files,
            backed_up: raw.backed_up,
            parameters: raw.parameters,
            policies: raw.policies,
        })
    }
}
```

Make sure `ResolvedParameter` and `ResolvedPolicy` derive `Serialize` and `Deserialize`. Add to `parameters.rs`:

```rust
// On ResolvedParameter:
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
```

And to `policies/mod.rs`:

```rust
// On ResolvedPolicy:
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
```

(Both already have the other traits; just add serde derives.)

- [ ] **Step 5: Wire into `activate_namespace`**

In `crates/aenv-core/src/activate/mod.rs`, after the `resolve_namespace` call, the resolution already contains parameters and policies. Update the `ActivationState` constructor:

```rust
let state = ActivationState {
    schema_version: SCHEMA_VERSION,
    active_namespace: leaf.as_str().to_owned(),
    project_root: project_root.to_path_buf(),
    managed_files: managed,
    backed_up,
    parameters: resolution.parameters.clone(),
    policies: resolution.policies.clone(),
};
```

Note: `resolution` is already in scope from `crate::resolve::resolve_namespace(...)`. The fields are not used by anything yet (Task 14 reads them); for now they're just persisted.

- [ ] **Step 6: Run the new tests + regression**

Run: `cargo test -p aenv-core --test resolution_with_params_and_policies --test state_schema 2>&1 | tail -10`
Expected: PASS — 4 tests passed.

Run: `cargo test 2>&1 | tail -10`
Expected: full workspace green. Adjust any `ResolutionResult { ... }` struct literals in tests that still need the two new fields.

- [ ] **Step 7: Commit**

```bash
git add crates/aenv-core/src/resolve.rs crates/aenv-core/src/state.rs crates/aenv-core/src/activate/mod.rs crates/aenv-core/src/parameters.rs crates/aenv-core/src/policies/mod.rs crates/aenv-core/src/policies/builtin/mod.rs crates/aenv-core/tests/resolution_with_params_and_policies.rs crates/aenv-core/tests/state_schema.rs
# Plus any test files updated for ResolutionResult literal sites.
git commit -m "Thread parameters and policies through ResolutionResult and state schema 3"
```

---

### Task 14: `doctor` orchestrator (library)

The CLI uses this in Task 17. This task builds the orchestrator: a function that takes a `ResolutionResult`, walks every resolved policy, dispatches each one to its evaluator, collects the outcomes, and rolls them up into a `DoctorReport`. The report also classifies every outcome by severity and tells callers whether any enforce-violation occurred (so activation can short-circuit before doing any work).

**Files:**
- Create: `crates/aenv-core/src/doctor.rs`
- Modify: `crates/aenv-core/src/lib.rs` — add `pub mod doctor;`
- Test: `crates/aenv-core/tests/doctor.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/doctor.rs`:

```rust
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::doctor::{evaluate, DoctorReport};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::OutcomeStatus;
use aenv_core::policies::{PolicyValue, ResolvedPolicy};
use aenv_core::resolve::{Candidate, ResolutionResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ns(s: &str) -> NamespaceId {
    NamespaceId::new(s).unwrap()
}

fn claude_adapter() -> Adapter {
    let mut roles = BTreeMap::new();
    roles.insert("CLAUDE.md".into(), "instructions".into());
    Adapter {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into()],
        merge_strategies: BTreeMap::new(),
        roles,
        default_merge: BTreeMap::new(),
        parameters: vec![],
    }
}

#[test]
fn clean_report_when_all_pass() {
    let fs = MockFilesystem::new();
    fs.write(
        &PathBuf::from("/h/envs/base/CLAUDE.md"),
        "small body".as_bytes(),
    )
    .unwrap();
    let mut adapters = AdapterRegistry::new();
    adapters.insert(claude_adapter());
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
        }],
        parameters: BTreeMap::new(),
        policies: BTreeMap::from([(
            "instructions_max_chars".into(),
            ResolvedPolicy {
                value: PolicyValue::Integer(5000),
                enforce: false,
                source: ns("base"),
            },
        )]),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    assert!(!report.has_enforce_violations());
    assert_eq!(report.fail_count(), 0);
    assert!(report.outcomes.iter().any(|o| matches!(o.status, OutcomeStatus::Pass)));
}

#[test]
fn enforce_violation_is_flagged() {
    let fs = MockFilesystem::new();
    fs.write(
        &PathBuf::from("/h/envs/base/CLAUDE.md"),
        "x".repeat(10000).as_bytes(),
    )
    .unwrap();
    let mut adapters = AdapterRegistry::new();
    adapters.insert(claude_adapter());
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![Candidate {
            namespace: ns("base"),
            path: PathBuf::from("CLAUDE.md"),
            source_path: PathBuf::from("/h/envs/base/CLAUDE.md"),
            adapter: "claude-code".into(),
            merge_override: None,
        }],
        parameters: BTreeMap::new(),
        policies: BTreeMap::from([(
            "instructions_max_chars".into(),
            ResolvedPolicy {
                value: PolicyValue::Integer(5000),
                enforce: true,
                source: ns("base"),
            },
        )]),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    assert!(report.has_enforce_violations());
    assert_eq!(report.fail_count(), 1);
    let summary = report.summary_line();
    assert!(summary.contains("1 enforce") || summary.contains("violation"));
}

#[test]
fn empty_policies_means_pass() {
    let fs = MockFilesystem::new();
    let adapters = AdapterRegistry::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let resolved = ResolutionResult {
        chain: vec![ns("base")],
        candidates: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
    };

    let report = evaluate(&fs, &layout, &adapters, &resolved);
    assert!(!report.has_enforce_violations());
    assert!(report.outcomes.is_empty());
}

#[test]
fn report_records_chain_and_namespace_count() {
    let fs = MockFilesystem::new();
    let adapters = AdapterRegistry::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let resolved = ResolutionResult {
        chain: vec![ns("base"), ns("leaf")],
        candidates: vec![],
        parameters: BTreeMap::new(),
        policies: BTreeMap::new(),
    };
    let report: DoctorReport = evaluate(&fs, &layout, &adapters, &resolved);
    assert_eq!(report.chain.len(), 2);
    assert_eq!(report.chain[1].as_str(), "leaf");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test doctor 2>&1 | tail -10`
Expected: FAIL — `aenv_core::doctor` doesn't exist.

- [ ] **Step 3: Implement the orchestrator**

Create `crates/aenv-core/src/doctor.rs`:

```rust
//! Doctor: evaluate every resolved policy against the resolved namespace.
//!
//! Phase 3 produces a `DoctorReport` carrying:
//! * the namespace chain (for the CLI to print "base → leaf")
//! * the resolved policy map (key → ResolvedPolicy)
//! * the flat outcome list (one or more entries per policy key)
//!
//! `aenv doctor` (Task 17) renders this as text. The `enforce_policies_block`
//! function (Task 15) uses `has_enforce_violations()` to short-circuit
//! activation before any file writes.

use crate::adapter::AdapterRegistry;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::policies::builtin::{dispatch, OutcomeStatus, PolicyContext, PolicyOutcome};
use crate::policies::ResolvedPolicy;
use crate::resolve::ResolutionResult;
use std::collections::BTreeMap;

/// The product of evaluating every policy against a resolved namespace.
#[derive(Debug, Clone)]
pub struct DoctorReport {
    /// Root → leaf order.
    pub chain: Vec<NamespaceId>,
    /// Effective policies (qualified by source).
    pub policies: BTreeMap<String, ResolvedPolicy>,
    /// One outcome per (policy, target) pair.
    pub outcomes: Vec<PolicyOutcome>,
}

impl DoctorReport {
    /// Total number of `Fail` outcomes (enforced-policy violations).
    pub fn fail_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, OutcomeStatus::Fail { .. }))
            .count()
    }

    /// Number of `Warn` outcomes (advisory violations).
    pub fn warn_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
            .count()
    }

    /// Whether any enforce-policy violation occurred.
    pub fn has_enforce_violations(&self) -> bool {
        self.fail_count() > 0
    }

    /// One-line summary for human-friendly text output.
    pub fn summary_line(&self) -> String {
        let f = self.fail_count();
        let w = self.warn_count();
        if f == 0 && w == 0 {
            "No issues found.".into()
        } else if f == 0 {
            format!(
                "{w} advisory issue{}; activation unaffected (doctor is advisory).",
                if w == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "{f} enforce-policy violation{}, {w} advisory issue{}.",
                if f == 1 { "" } else { "s" },
                if w == 1 { "" } else { "s" }
            )
        }
    }
}

/// Evaluate every policy in `resolved.policies` against `resolved.candidates`.
pub fn evaluate<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    resolved: &ResolutionResult,
) -> DoctorReport {
    let ctx = PolicyContext {
        fs,
        layout,
        adapters,
        resolved,
    };
    let mut outcomes: Vec<PolicyOutcome> = Vec::new();
    for (key, policy) in &resolved.policies {
        outcomes.extend(dispatch(key, policy, &ctx));
    }
    DoctorReport {
        chain: resolved.chain.clone(),
        policies: resolved.policies.clone(),
        outcomes,
    }
}
```

Add `pub mod doctor;` to `crates/aenv-core/src/lib.rs`.

- [ ] **Step 4: Run the new test + regression**

Run: `cargo test -p aenv-core --test doctor 2>&1 | tail -10`
Expected: PASS — 4 tests passed.

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/doctor.rs crates/aenv-core/src/lib.rs crates/aenv-core/tests/doctor.rs
git commit -m "Add DoctorReport + evaluate() orchestrator"
```

---

### Task 15: Block activation on enforce-violations (R-74)

`aenv activate` must refuse when any `enforce = true` policy is violated (PRD R-74 / R-82 — exit 17). The block happens *before* `activate_namespace` does any backups or symlinks, so the project is exactly as it was on rejection (R-63).

**Files:**
- Modify: `crates/aenv-core/src/activate/mod.rs` — run doctor before materialization
- Test: `crates/aenv-core/tests/activate_enforce.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/activate_enforce.rs`:

```rust
use aenv_core::activate::activate_namespace;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::error::AenvError;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn make_layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/h"))
}

fn install_claude_adapter(fs: &MockFilesystem, layout: &RegistryLayout) -> AdapterRegistry {
    let toml = r#"
name = "claude-code"
files = ["CLAUDE.md"]

[roles]
"CLAUDE.md" = "instructions"
"#;
    fs.write(
        &layout.adapters_dir().join("claude-code.toml"),
        toml.as_bytes(),
    )
    .unwrap();
    AdapterRegistry::load_from_dir(fs, &layout.adapters_dir()).unwrap()
}

fn write_manifest(fs: &MockFilesystem, layout: &RegistryLayout, name: &str, body: &str) {
    fs.write(&layout.manifest_path(name), body.as_bytes()).unwrap();
}

#[test]
fn activation_refused_when_enforce_violation() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = install_claude_adapter(&fs, &layout);

    let manifest = r#"
name = "tight"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = { value = 100, enforce = true }
"#;
    write_manifest(&fs, &layout, "tight", manifest);
    let body = "x".repeat(500);
    fs.write(
        &layout.namespace_dir("tight").join("CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let err = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("tight").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, AenvError::PolicyViolation(_)));
    assert_eq!(err.exit_code(), 17);
    // No state file should have been written.
    assert!(!fs.exists(&project.join(".aenv-state/state.json")).unwrap());
    // No symlink should exist in the project.
    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn advisory_violation_does_not_block_activation() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = install_claude_adapter(&fs, &layout);

    let manifest = r#"
name = "loose"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = 100
"#;
    write_manifest(&fs, &layout, "loose", manifest);
    let body = "x".repeat(500);
    fs.write(
        &layout.namespace_dir("loose").join("CLAUDE.md"),
        body.as_bytes(),
    )
    .unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("loose").unwrap(),
    )
    .unwrap();
    assert!(fs.exists(&project.join(".aenv-state/state.json")).unwrap());
}

#[test]
fn enforce_violation_message_names_policy_and_namespace() {
    let fs = MockFilesystem::new();
    let layout = make_layout();
    let adapters = install_claude_adapter(&fs, &layout);
    let manifest = r#"
name = "tight"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = { value = 100, enforce = true }
"#;
    write_manifest(&fs, &layout, "tight", manifest);
    fs.write(
        &layout.namespace_dir("tight").join("CLAUDE.md"),
        "x".repeat(500).as_bytes(),
    )
    .unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let err = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("tight").unwrap(),
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("instructions_max_chars"), "msg = {msg}");
    assert!(msg.contains("100"), "msg = {msg}");
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-core --test activate_enforce 2>&1 | tail -10`
Expected: FAIL — activation currently doesn't check enforce policies.

- [ ] **Step 3: Insert the enforce-block**

In `crates/aenv-core/src/activate/mod.rs`, after `let resolution = crate::resolve::resolve_namespace(...)?;` and before the `let mut by_path = ...;` line, add:

```rust
// PRD R-74 / R-82: run doctor before materializing anything. If any enforced
// policy is violated, abort with PolicyViolation (exit 17) so the project
// stays exactly as we found it (R-63).
let report = crate::doctor::evaluate(fs, layout, adapters, &resolution);
if report.has_enforce_violations() {
    let mut details: Vec<String> = Vec::new();
    for o in &report.outcomes {
        if let crate::policies::builtin::OutcomeStatus::Fail { msg } = &o.status {
            let target_label = o
                .target
                .as_ref()
                .map(|qn| qn.to_string())
                .unwrap_or_else(|| "<namespace>".to_string());
            details.push(format!("[{}] {target_label}: {msg}", o.key));
        }
    }
    return Err(AenvError::PolicyViolation(details.join("; ")));
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-core --test activate_enforce 2>&1 | tail -10`
Expected: PASS — 3 tests passed.

Run: `cargo test 2>&1 | tail -10`
Expected: full workspace green. (The earlier Phase 2 activation e2e tests do not exercise enforce-violations, so they should be unaffected.)

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-core/src/activate/mod.rs crates/aenv-core/tests/activate_enforce.rs
git commit -m "Block activation on enforce-policy violation (exit 17)"
```

---

### Task 16: `aenv get` command

Two forms (PRD R-69 + spec §5.5):

```
aenv get <namespace>.<parameter>      # explicit namespace
aenv get .<parameter>                 # active namespace in current project
```

Output:

```
<value>
  source: <namespace> (overrides <prev_namespace> which declared <prev_value>)
```

For a parameter that exists only at one level, the second line is `source: <namespace> (declared, not inherited)` or `source: <namespace> (inherited, not overridden)` depending on whether the source is the leaf or an ancestor.

Exit code 16 (`ParameterUndefined`) if the parameter is not declared anywhere in the chain. Exit code 10 if the named namespace does not exist.

**Files:**
- Create: `crates/aenv-cli/src/cmd/get.rs`
- Modify: `crates/aenv-cli/src/cmd/mod.rs` — `pub mod get;`
- Modify: `crates/aenv-cli/src/main.rs` — add `Get { spec: String }` subcommand
- Test: `crates/aenv-cli/tests/get_e2e.rs`

- [ ] **Step 1: Write the failing CLI test**

Create `crates/aenv-cli/tests/get_e2e.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[path = "common/mod.rs"]
mod common;
use common::TestEnv;

#[test]
fn get_active_project_parameter() {
    let env = TestEnv::new();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-haiku-4.5"
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/base/CLAUDE.md"), "hi").unwrap();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["use", "base"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["get", ".default_model"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success()
        .stdout(predicate::str::contains("claude-haiku-4.5"))
        .stdout(predicate::str::contains("source: base"));
}

#[test]
fn get_with_explicit_namespace_shows_override_provenance() {
    let env = TestEnv::new();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "leaf"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-haiku-4.5"
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/base/CLAUDE.md"), "hi").unwrap();
    fs::write(
        env.aenv_home().join("envs/leaf/aenv.toml"),
        r#"
name = "leaf"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-opus-4.7"
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/leaf/CLAUDE.md"), "leaf").unwrap();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["get", "leaf.default_model"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success()
        .stdout(predicate::str::contains("claude-opus-4.7"))
        .stdout(predicate::str::contains("source: leaf"))
        .stdout(predicate::str::contains("overrides base"))
        .stdout(predicate::str::contains("claude-haiku-4.5"));
}

#[test]
fn get_undefined_parameter_exits_16() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["get", "base.nonexistent"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .code(16)
        .stderr(predicate::str::contains("nonexistent"));
}

#[test]
fn get_active_when_no_project_pinned_exits_20() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["get", ".default_model"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .code(20);
}
```

Make sure `common/mod.rs` exists alongside; Phase 2 created it. Reuse.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-cli --test get_e2e 2>&1 | tail -10`
Expected: FAIL — `get` subcommand doesn't exist.

- [ ] **Step 3: Implement `cmd/get.rs`**

Create `crates/aenv-cli/src/cmd/get.rs`:

```rust
//! `aenv get <ns>.<param>` and `aenv get .<param>` — print effective value
//! plus qualified provenance (PRD R-69, spec §5.5).

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::AenvError;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::resolve_namespace;
use aenv_core::state::ActivationState;
use aenv_core::Result;
use std::path::Path;

/// `aenv get <spec>`. `spec` is either `<namespace>.<parameter>` or `.<parameter>`.
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    spec: &str,
) -> Result<()> {
    let (ns_name, param) = parse_spec(spec)?;
    let leaf: NamespaceId = match ns_name {
        Some(name) => NamespaceId::new(name)
            .map_err(|e| AenvError::ManifestInvalid(format!("invalid namespace: {e}")))?,
        None => active_namespace(fs, project_root)?,
    };

    let resolution = resolve_namespace(fs, layout, adapters, &leaf)?;
    let rp = resolution.parameters.get(param).ok_or_else(|| {
        AenvError::ParameterUndefined(format!("{}.{}", leaf, param))
    })?;

    // Find any prior contributor (same key, earlier in the chain) for the
    // "overrides" provenance line.
    let mut prior: Option<(NamespaceId, String)> = None;
    let leaf_id = leaf.clone();
    for ns in &resolution.chain {
        if *ns == leaf_id && rp.source == leaf_id {
            break;
        }
        // Re-read just enough to inspect this namespace's [parameters] entry.
        let manifest_path = layout.manifest_path(ns.as_str());
        let bytes = match fs.read(&manifest_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let Ok(m) = aenv_core::manifest::AenvManifest::from_toml(text) {
            if let Some(v) = m.parameters.get(param) {
                if *ns != rp.source {
                    prior = Some((ns.clone(), v.to_string()));
                }
            }
        }
    }

    println!("{}", rp.value);
    match (rp.source == leaf_id, prior) {
        (true, Some((p_ns, p_value))) => {
            println!(
                "  source: {} (overrides {} which declared {})",
                rp.source, p_ns, p_value
            );
        }
        (true, None) => {
            println!("  source: {} (declared, not inherited)", rp.source);
        }
        (false, _) => {
            println!("  source: {} (inherited, not overridden)", rp.source);
        }
    }
    Ok(())
}

fn parse_spec(spec: &str) -> Result<(Option<&str>, &str)> {
    if let Some(stripped) = spec.strip_prefix('.') {
        if stripped.is_empty() {
            return Err(AenvError::ManifestInvalid(
                "empty parameter name after leading '.'".into(),
            ));
        }
        Ok((None, stripped))
    } else {
        let (ns, param) = spec
            .split_once('.')
            .ok_or_else(|| AenvError::ManifestInvalid(format!(
                "expected '<namespace>.<parameter>' or '.<parameter>', got '{spec}'"
            )))?;
        if ns.is_empty() || param.is_empty() {
            return Err(AenvError::ManifestInvalid(format!(
                "invalid spec '{spec}': both namespace and parameter must be non-empty"
            )));
        }
        Ok((Some(ns), param))
    }
}

fn active_namespace<F: Filesystem>(fs: &F, project_root: &Path) -> Result<NamespaceId> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        return Err(AenvError::ProjectNotPinned);
    }
    let bytes = fs.read(&state_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("state file not UTF-8: {e}")))?;
    let state = ActivationState::from_json(text)?;
    NamespaceId::new(state.active_namespace.as_str())
        .map_err(|e| AenvError::ManifestInvalid(format!("invalid active namespace: {e}")))
}
```

Wire into `crates/aenv-cli/src/cmd/mod.rs`:

```rust
pub mod get;
```

In `crates/aenv-cli/src/main.rs`, add to the clap enum:

```rust
/// Print the effective value of a parameter.
Get {
    /// Either `<namespace>.<parameter>` or `.<parameter>` (active project).
    spec: String,
},
```

And in the dispatch:

```rust
Command::Get { spec } => {
    cmd::get::run(&fs, &layout, &adapters, &project_root, &spec)?;
}
```

(Match the exact dispatch pattern Phase 2 uses for other commands.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p aenv-cli --test get_e2e 2>&1 | tail -20`
Expected: PASS — 4 tests passed.

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-cli/src/cmd/get.rs crates/aenv-cli/src/cmd/mod.rs crates/aenv-cli/src/main.rs crates/aenv-cli/tests/get_e2e.rs
git commit -m "Add 'aenv get <spec>' command"
```

---

### Task 17: `aenv set` command

Writes a parameter into the named namespace's `aenv.toml` `[parameters]` table (PRD R-70). Always requires an explicit namespace — `aenv set <namespace>.<parameter> <value>`. If the parameter already exists in that namespace, it's overwritten; otherwise it's inserted. The value's type is inferred from the input string:

- `true` / `false` (case-insensitive) → boolean
- All-digit (optional leading `-`) → integer
- `[a, b, c]` (basic comma-separated, optionally quoted) → list-of-string
- Everything else → string

This keeps the CLI ergonomic without sprawling. Users who want unambiguous typing can edit the TOML file directly.

After the write, the new manifest is re-parsed to verify it still parses (catches the case where the new value would create a type collision against another adapter declaration via R-71 — the resolver/check phase, not the file write itself, surfaces the failure).

**Files:**
- Create: `crates/aenv-cli/src/cmd/set.rs`
- Modify: `crates/aenv-cli/src/cmd/mod.rs` — `pub mod set;`
- Modify: `crates/aenv-cli/src/main.rs` — add `Set { spec, value }` subcommand
- Test: `crates/aenv-cli/tests/set_e2e.rs`

- [ ] **Step 1: Write the failing CLI test**

Create `crates/aenv-cli/tests/set_e2e.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[path = "common/mod.rs"]
mod common;
use common::TestEnv;

#[test]
fn set_inserts_new_parameter() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["set", "base.default_model", "claude-opus-4.7"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    let manifest = fs::read_to_string(env.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(manifest.contains("default_model"));
    assert!(manifest.contains("claude-opus-4.7"));
}

#[test]
fn set_overwrites_existing() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[parameters]
budget = 5000
"#,
    )
    .unwrap();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["set", "base.budget", "3000"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    let manifest = fs::read_to_string(env.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(manifest.contains("3000"));
    assert!(!manifest.contains("5000"));
}

#[test]
fn set_infers_boolean() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["set", "base.verbose", "true"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    let manifest = fs::read_to_string(env.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(
        manifest.contains("verbose = true"),
        "manifest = {manifest}"
    );
}

#[test]
fn set_infers_list_of_strings() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["set", "base.forbid_tools", "[edit, write, bash:rm]"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    let manifest = fs::read_to_string(env.aenv_home().join("envs/base/aenv.toml")).unwrap();
    assert!(manifest.contains("edit"));
    assert!(manifest.contains("write"));
    assert!(manifest.contains("bash:rm"));
}

#[test]
fn set_unknown_namespace_exits_10() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["set", "ghost.x", "1"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .code(10);
}

#[test]
fn set_requires_explicit_namespace() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["set", ".x", "1"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .failure()
        .stderr(predicate::str::contains("namespace"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-cli --test set_e2e 2>&1 | tail -10`
Expected: FAIL — `set` subcommand doesn't exist.

- [ ] **Step 3: Implement `cmd/set.rs`**

Create `crates/aenv-cli/src/cmd/set.rs`:

```rust
//! `aenv set <namespace>.<parameter> <value>` — write a parameter into the
//! named namespace's manifest (PRD R-70). Value type is inferred from the
//! literal.

use aenv_core::error::AenvError;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::parameters::ParameterValue;
use aenv_core::Result;

/// Entry point.
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    spec: &str,
    value_literal: &str,
) -> Result<()> {
    let (ns, param) = parse_spec(spec)?;
    let value = infer_value(value_literal);

    let manifest_path = layout.manifest_path(ns);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(ns.into()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("manifest not UTF-8: {e}")))?;
    let mut manifest = AenvManifest::from_toml(text)?;
    manifest.parameters.insert(param.to_string(), value);

    // Verify it still parses after the round-trip.
    let rendered = manifest.to_toml();
    let _ = AenvManifest::from_toml(&rendered)?;

    fs.write(&manifest_path, rendered.as_bytes())?;
    println!("Set {ns}.{param}");
    Ok(())
}

fn parse_spec(spec: &str) -> Result<(&str, &str)> {
    if spec.starts_with('.') {
        return Err(AenvError::ManifestInvalid(
            "'set' requires an explicit namespace: `aenv set <namespace>.<parameter> <value>`".into(),
        ));
    }
    let (ns, param) = spec.split_once('.').ok_or_else(|| {
        AenvError::ManifestInvalid(format!(
            "expected '<namespace>.<parameter>', got '{spec}'"
        ))
    })?;
    if ns.is_empty() || param.is_empty() {
        return Err(AenvError::ManifestInvalid(format!(
            "invalid spec '{spec}': both namespace and parameter must be non-empty"
        )));
    }
    Ok((ns, param))
}

fn infer_value(literal: &str) -> ParameterValue {
    let trimmed = literal.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return ParameterValue::Boolean(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return ParameterValue::Boolean(false);
    }
    if let Ok(n) = trimmed.parse::<i64>() {
        return ParameterValue::Integer(n);
    }
    if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let xs: Vec<String> = inner
            .split(',')
            .map(|item| {
                let t = item.trim();
                t.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(t)
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();
        return ParameterValue::ListString(xs);
    }
    ParameterValue::String(trimmed.to_string())
}
```

Wire into `cmd/mod.rs`:

```rust
pub mod set;
```

In `main.rs`, add the clap variant:

```rust
/// Set a parameter on a namespace.
Set {
    /// `<namespace>.<parameter>`
    spec: String,
    /// Value literal (type inferred: true/false → bool, digits → int,
    /// "[a, b]" → list-of-string, else string).
    value: String,
},
```

And the dispatch:

```rust
Command::Set { spec, value } => {
    cmd::set::run(&fs, &layout, &spec, &value)?;
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p aenv-cli --test set_e2e 2>&1 | tail -10`
Expected: PASS — 6 tests passed.

Run: `cargo test 2>&1 | tail -5`
Expected: full workspace green.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-cli/src/cmd/set.rs crates/aenv-cli/src/cmd/mod.rs crates/aenv-cli/src/main.rs crates/aenv-cli/tests/set_e2e.rs
git commit -m "Add 'aenv set <ns>.<param> <value>' command"
```

---

### Task 18: `aenv doctor` command + upgrade `aenv status`

Two CLI changes in one task because they share the same rendering helpers:

- **`aenv doctor [<namespace>]`** — resolves the namespace (defaults to the active project's leaf), runs the `doctor::evaluate` orchestrator, prints the report, and exits with 17 if any `enforce = true` policy was violated. Without an explicit namespace argument, falls back on the active project's pinned namespace; if no project is pinned and no argument given, exit 20 (`ProjectNotPinned`).
- **`aenv status`** — append "Parameters" and "Active policies" sections after the existing resolution-chain output. Pull both from the persisted `ActivationState`, so `aenv status` works without re-resolving.

**Files:**
- Create: `crates/aenv-cli/src/cmd/doctor.rs`
- Modify: `crates/aenv-cli/src/cmd/status.rs` — append two new sections
- Modify: `crates/aenv-cli/src/cmd/mod.rs` — `pub mod doctor;`
- Modify: `crates/aenv-cli/src/main.rs` — add `Doctor { ns: Option<String> }` subcommand
- Test: `crates/aenv-cli/tests/doctor_e2e.rs`
- Test: `crates/aenv-cli/tests/status_with_params.rs`

- [ ] **Step 1: Write the failing CLI tests**

Create `crates/aenv-cli/tests/doctor_e2e.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[path = "common/mod.rs"]
mod common;
use common::TestEnv;

#[test]
fn doctor_reports_clean_when_no_violations() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = 5000
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/base/CLAUDE.md"), "short body").unwrap();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["doctor", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Namespace 'base'"))
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
fn doctor_reports_advisory_violation_zero_exit() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    let body = "x".repeat(8000);
    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = 5000
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/base/CLAUDE.md"), body).unwrap();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["doctor", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success() // advisory only — exit 0
        .stdout(predicate::str::contains("POLICY"))
        .stdout(predicate::str::contains("instructions_max_chars"))
        .stdout(predicate::str::contains("8000"));
}

#[test]
fn doctor_exits_17_on_enforce_violation() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "tight"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    let body = "x".repeat(8000);
    fs::write(
        env.aenv_home().join("envs/tight/aenv.toml"),
        r#"
name = "tight"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = { value = 5000, enforce = true }
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/tight/CLAUDE.md"), body).unwrap();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["doctor", "tight"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .code(17)
        .stdout(predicate::str::contains("instructions_max_chars"));
}

#[test]
fn doctor_with_no_arg_uses_active_project() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/base/CLAUDE.md"), "ok").unwrap();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["use", "base"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["doctor"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Namespace 'base'"));
}

#[test]
fn doctor_with_no_arg_no_pin_exits_20() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["doctor"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .code(20);
}
```

Create `crates/aenv-cli/tests/status_with_params.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[path = "common/mod.rs"]
mod common;
use common::TestEnv;

#[test]
fn status_prints_parameters_section() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "haiku"
budget = 5000

[policies]
skill_requires_description = true
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/base/CLAUDE.md"), "ok").unwrap();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["use", "base"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["status"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Parameters"))
        .stdout(predicate::str::contains("default_model"))
        .stdout(predicate::str::contains("haiku"))
        .stdout(predicate::str::contains("budget"))
        .stdout(predicate::str::contains("Active policies"))
        .stdout(predicate::str::contains("skill_requires_description"));
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p aenv-cli --test doctor_e2e --test status_with_params 2>&1 | tail -20`
Expected: FAIL — `doctor` subcommand doesn't exist; `status` doesn't render new sections.

- [ ] **Step 3: Implement `cmd/doctor.rs`**

Create `crates/aenv-cli/src/cmd/doctor.rs`:

```rust
//! `aenv doctor [<namespace>]` — evaluate every resolved policy and print
//! per-policy outcomes. Exits 17 if any `enforce = true` policy is violated;
//! exits 0 otherwise (advisory warnings do not change the exit code).

use aenv_core::adapter::AdapterRegistry;
use aenv_core::doctor::{evaluate, DoctorReport};
use aenv_core::error::AenvError;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::OutcomeStatus;
use aenv_core::resolve::resolve_namespace;
use aenv_core::state::ActivationState;
use aenv_core::Result;
use std::path::Path;

/// Entry point.
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    ns_arg: Option<&str>,
) -> Result<()> {
    let leaf: NamespaceId = match ns_arg {
        Some(name) => NamespaceId::new(name)
            .map_err(|e| AenvError::ManifestInvalid(format!("invalid namespace: {e}")))?,
        None => active_namespace(fs, project_root)?,
    };

    let resolution = resolve_namespace(fs, layout, adapters, &leaf)?;
    let report = evaluate(fs, layout, adapters, &resolution);
    print_report(&leaf, &resolution.chain, &report);

    if report.has_enforce_violations() {
        // Surface the failure message via AenvError so the CLI driver maps
        // it to exit 17 consistently.
        return Err(AenvError::PolicyViolation(report.summary_line()));
    }
    Ok(())
}

fn active_namespace<F: Filesystem>(fs: &F, project_root: &Path) -> Result<NamespaceId> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        return Err(AenvError::ProjectNotPinned);
    }
    let bytes = fs.read(&state_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("state not UTF-8: {e}")))?;
    let state = ActivationState::from_json(text)?;
    NamespaceId::new(state.active_namespace.as_str())
        .map_err(|e| AenvError::ManifestInvalid(format!("invalid active namespace: {e}")))
}

fn print_report(leaf: &NamespaceId, chain: &[NamespaceId], report: &DoctorReport) {
    let chain_str: Vec<String> = chain.iter().map(|n| n.as_str().to_owned()).collect();
    println!(
        "Namespace '{leaf}' (resolution: {})",
        chain_str.join(" -> ")
    );
    println!();
    if report.policies.is_empty() {
        println!("Active policies: (none)");
    } else {
        println!("Active policies (after inheritance):");
        for (k, p) in &report.policies {
            let enforce = if p.enforce { " enforce=true" } else { "" };
            println!("  {k:30} = {} (from {}){}", p.value_display(), p.source, enforce);
        }
    }
    println!();

    if report.outcomes.is_empty() {
        println!("{}", report.summary_line());
        return;
    }

    let mut passes = 0usize;
    let mut fails: Vec<&aenv_core::policies::builtin::PolicyOutcome> = Vec::new();
    let mut warns: Vec<&aenv_core::policies::builtin::PolicyOutcome> = Vec::new();
    let mut skips: Vec<&aenv_core::policies::builtin::PolicyOutcome> = Vec::new();
    for o in &report.outcomes {
        match &o.status {
            OutcomeStatus::Pass => passes += 1,
            OutcomeStatus::Warn { .. } => warns.push(o),
            OutcomeStatus::Fail { .. } => fails.push(o),
            OutcomeStatus::WarnSkip { .. } => skips.push(o),
        }
    }
    if !fails.is_empty() {
        println!("Issues:");
        for o in &fails {
            if let OutcomeStatus::Fail { msg } = &o.status {
                let target_label = o
                    .target
                    .as_ref()
                    .map(|qn| qn.to_string())
                    .unwrap_or_default();
                println!("  X POLICY violation: {}", o.key);
                if !target_label.is_empty() {
                    println!("    target: {target_label}");
                }
                for line in msg.lines() {
                    println!("    {line}");
                }
            }
        }
    }
    if !warns.is_empty() {
        if !fails.is_empty() {
            println!();
        }
        println!("Advisory:");
        for o in &warns {
            if let OutcomeStatus::Warn { msg } = &o.status {
                let target_label = o
                    .target
                    .as_ref()
                    .map(|qn| qn.to_string())
                    .unwrap_or_default();
                println!("  ! POLICY {}", o.key);
                if !target_label.is_empty() {
                    println!("    target: {target_label}");
                }
                for line in msg.lines() {
                    println!("    {line}");
                }
            }
        }
    }
    if !skips.is_empty() {
        println!();
        println!("Skipped:");
        for o in &skips {
            if let OutcomeStatus::WarnSkip { msg } = &o.status {
                println!("  - {} ({msg})", o.key);
            }
        }
    }
    println!();
    println!("{} pass, {} warn, {} fail, {} skipped.", passes, warns.len(), fails.len(), skips.len());
    println!("{}", report.summary_line());
}
```

The `value_display` helper needs to exist on `ResolvedPolicy`. Add to `policies/mod.rs`:

```rust
impl ResolvedPolicy {
    /// Human-readable rendering of the value side.
    pub fn value_display(&self) -> String {
        match &self.value {
            PolicyValue::Integer(i) => i.to_string(),
            PolicyValue::Boolean(b) => b.to_string(),
            PolicyValue::ListString(xs) => format!(
                "[{}]",
                xs.iter()
                    .map(|s| format!("\"{s}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}
```

Wire `doctor` into `cmd/mod.rs`:

```rust
pub mod doctor;
```

In `main.rs`:

```rust
/// Evaluate policies and report (PRD R-73).
Doctor {
    /// Optional namespace name. Defaults to the active project's pin.
    namespace: Option<String>,
},
```

Dispatch:

```rust
Command::Doctor { namespace } => {
    cmd::doctor::run(
        &fs,
        &layout,
        &adapters,
        &project_root,
        namespace.as_deref(),
    )?;
}
```

- [ ] **Step 4: Upgrade `cmd/status.rs`**

Open `crates/aenv-cli/src/cmd/status.rs`. After the existing block that prints the resolution chain and managed files, append:

```rust
if !state.parameters.is_empty() {
    println!();
    println!("Parameters:");
    for (k, rp) in &state.parameters {
        println!("  {k:30} = {} (from {})", rp.value, rp.source);
    }
}
if !state.policies.is_empty() {
    println!();
    println!("Active policies:");
    for (k, rp) in &state.policies {
        let enforce = if rp.enforce { " enforce=true" } else { "" };
        println!(
            "  {k:30} = {} (from {}){}",
            rp.value_display(),
            rp.source,
            enforce
        );
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p aenv-cli --test doctor_e2e 2>&1 | tail -20`
Expected: PASS — 5 tests passed.

Run: `cargo test -p aenv-cli --test status_with_params 2>&1 | tail -10`
Expected: PASS — 1 test passed.

Run: `cargo test 2>&1 | tail -10`
Expected: full workspace green.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-cli/src/cmd/doctor.rs crates/aenv-cli/src/cmd/mod.rs crates/aenv-cli/src/main.rs crates/aenv-cli/src/cmd/status.rs crates/aenv-core/src/policies/mod.rs crates/aenv-cli/tests/doctor_e2e.rs crates/aenv-cli/tests/status_with_params.rs
git commit -m "Add 'aenv doctor' command and upgrade 'aenv status' with parameters and policies"
```

---

### Task 19: End-to-end integration test (spec §5.5 + §5.12)

This is the contract test: it asserts the user-facing strings from the functional spec actually come out of the CLI for clean and violation scenarios. If a future refactor drifts the wording, this test catches it.

**Files:**
- Create: `crates/aenv-cli/tests/parameters_policies_e2e.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-cli/tests/parameters_policies_e2e.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[path = "common/mod.rs"]
mod common;
use common::TestEnv;

/// Reproduce the spec §5.5 example chain (base → detailed-execution) and
/// verify the parameter-query examples.
#[test]
fn provenance_walk_matches_spec_5_5() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "detailed-execution"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();

    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-sonnet-4.6"
instructions_budget = 5000

[policies]
skill_requires_description = true
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/base/CLAUDE.md"), "base body").unwrap();
    fs::write(
        env.aenv_home().join("envs/detailed-execution/aenv.toml"),
        r#"
name = "detailed-execution"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-opus-4.7"
instructions_budget = 3000
"#,
    )
    .unwrap();
    fs::write(
        env.aenv_home().join("envs/detailed-execution/CLAUDE.md"),
        "leaf body",
    )
    .unwrap();

    // explicit-namespace get: spec example $ aenv get .default_model -> opus + overrides base
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["get", "detailed-execution.default_model"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success()
        .stdout(predicate::str::contains("claude-opus-4.7"))
        .stdout(predicate::str::contains("source: detailed-execution"))
        .stdout(predicate::str::contains("overrides base"))
        .stdout(predicate::str::contains("claude-sonnet-4.6"));

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["get", "detailed-execution.instructions_budget"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success()
        .stdout(predicate::str::contains("3000"))
        .stdout(predicate::str::contains("source: detailed-execution"))
        .stdout(predicate::str::contains("overrides base"))
        .stdout(predicate::str::contains("5000"));

    // Inherited not overridden: detailed-execution.skill_requires_description
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["get", "detailed-execution.skill_requires_description"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .code(16); // it's a *policy*, not a parameter — `get` is parameter-only.

    // Doctor on the leaf should run cleanly (no body exceeds budget; nothing else to trip)
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["doctor", "detailed-execution"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Active policies"))
        .stdout(predicate::str::contains("skill_requires_description"));
}

/// Reproduce spec §5.12 violation example (CLAUDE.md too long + missing
/// skill description).
#[test]
fn doctor_violation_matches_spec_5_12() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "base"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "experiments-overgrown"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    fs::write(
        env.aenv_home().join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = 5000
skill_requires_description = true
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/base/CLAUDE.md"), "ok").unwrap();

    let big = "x".repeat(8247);
    fs::write(
        env.aenv_home().join("envs/experiments-overgrown/aenv.toml"),
        r#"
name = "experiments-overgrown"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md", ".claude/skills/half-baked-skill/SKILL.md"]
"#,
    )
    .unwrap();
    fs::write(
        env.aenv_home().join("envs/experiments-overgrown/CLAUDE.md"),
        big,
    )
    .unwrap();
    fs::create_dir_all(
        env.aenv_home()
            .join("envs/experiments-overgrown/.claude/skills/half-baked-skill"),
    )
    .unwrap();
    fs::write(
        env.aenv_home().join(
            "envs/experiments-overgrown/.claude/skills/half-baked-skill/SKILL.md",
        ),
        "---\nname: half-baked-skill\n---\nNo description.\n",
    )
    .unwrap();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["doctor", "experiments-overgrown"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success() // advisory — exit 0
        .stdout(predicate::str::contains("POLICY"))
        .stdout(predicate::str::contains("instructions_max_chars"))
        .stdout(predicate::str::contains("8247"))
        .stdout(predicate::str::contains("5000"))
        .stdout(predicate::str::contains("skill_requires_description"))
        .stdout(predicate::str::contains("half-baked-skill"));
}

/// When `instructions_max_chars` is marked enforce, activation refuses.
/// The state file should not appear in the project.
#[test]
fn activate_refused_when_enforce_violation() {
    let env = TestEnv::new();
    Command::cargo_bin("aenv")
        .unwrap()
        .args(["create", "tight"])
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .success();
    let big = "x".repeat(8000);
    fs::write(
        env.aenv_home().join("envs/tight/aenv.toml"),
        r#"
name = "tight"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = { value = 5000, enforce = true }
"#,
    )
    .unwrap();
    fs::write(env.aenv_home().join("envs/tight/CLAUDE.md"), big).unwrap();

    Command::cargo_bin("aenv")
        .unwrap()
        .args(["use", "tight"])
        .current_dir(env.project())
        .env("AENV_HOME", env.aenv_home())
        .assert()
        .code(17);

    assert!(!env.project().join(".aenv-state/state.json").exists());
    assert!(!env.project().join("CLAUDE.md").exists());
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p aenv-cli --test parameters_policies_e2e 2>&1 | tail -30`
Expected: PASS — 3 tests passed.

Run the entire workspace one more time to be safe:

Run: `cargo test 2>&1 | tail -10`
Expected: full workspace green.

- [ ] **Step 3: Run `cargo fmt`**

Run: `cargo fmt`
Expected: silent.

Run: `git status` — expect to see formatting changes across the new files. If any whole-line diffs appear, examine and stage them.

- [ ] **Step 4: Run `cargo clippy --all-targets`**

Run: `cargo clippy --all-targets 2>&1 | tail -20`
Expected: no warnings. If clippy flags new warnings (e.g. `needless_borrow`, `redundant_closure`), fix them inline before committing.

- [ ] **Step 5: Commit**

```bash
git add crates/aenv-cli/tests/parameters_policies_e2e.rs
# plus any cargo-fmt churn
git add -u
git commit -m "Add end-to-end parameters and policies integration tests + cargo fmt"
```

---

### Task 20: Tag `phase-3-complete`

The final mile-marker. Smoke-test the whole workspace, write a CHANGELOG entry into the tag, and push the milestone.

- [ ] **Step 1: Final regression**

Run: `cargo test --workspace --all-targets 2>&1 | tail -20`
Expected: every test passes; no warnings.

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20`
Expected: silent.

Run: `cargo fmt --check`
Expected: silent.

- [ ] **Step 2: Tag**

```bash
git tag -a phase-3-complete -m "$(cat <<'EOF'
Phase 3 complete: parameters & policies

Deliverable:
- [parameters] block in manifests: typed (string/int/bool/list-of-string),
  inherited last-wins across the extends chain, recorded in state.json.
- [policies] block: shorthand + table-form parse, advisory by default,
  enforce-protection (R-75) on inheritance.
- Four built-in policy evaluators: instructions_max_chars,
  skill_requires_description, mcp_requires_command_or_url, forbid_paths.
- CLI: `aenv get <ns>.<param>` / `.<param>`, `aenv set <ns>.<param> <value>`,
  `aenv doctor [<ns>]`. `aenv status` now prints parameters + active policies.
- Activation blocks on enforce-violations (exit 17) before any file write.
- State schema bumped to 3; schema 2 read-compat preserved.

Covers PRD: R-24, R-25, R-26, R-27, R-28, R-66, R-67, R-68, R-69, R-70, R-71,
R-72, R-73, R-74, R-75.

Covers functional spec: §5.5 (parameter queries), §5.12 (doctor clean +
violation cases).

Deliberately deferred to later phases:
- Adapter parameter projection into tool-specific configs (Phase 4)
- aenv doctor --json (Phase 5)
- Adapter-specific policy keys beyond the four built-ins (later)
- Soft size limits as default policies when no manifest declares them (later)
EOF
)"
```

- [ ] **Step 3: Verify tag**

Run: `git tag -l --format='%(contents:subject)' phase-3-complete`
Expected: `Phase 3 complete: parameters & policies`

Run: `git log --oneline phase-2-complete..phase-3-complete | wc -l`
Expected: roughly 19 commits (one per task).

---

## Phase 3 completion check

After Task 20:

- [ ] Every checkbox in this plan is checked.
- [ ] `cargo test --workspace --all-targets` is green.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is silent.
- [ ] `cargo fmt --check` is silent.
- [ ] `phase-3-complete` git tag points at the final commit.
- [ ] State schema is 3; an existing schema-2 state file from Phase 2 still loads.
- [ ] PRD requirements R-24, R-25, R-26, R-27, R-28, R-66, R-67, R-68, R-69, R-70, R-71, R-72, R-73, R-74, R-75 all have a corresponding test that exercises the requirement.
- [ ] Functional spec §5.5 parameter queries and §5.12 doctor (clean + violation) match the CLI output (asserted by `parameters_policies_e2e.rs`).

If any criterion fails, fix it in a follow-up commit and re-tag (delete + recreate `phase-3-complete`).

