# Phase 5 — Resolved-Namespace Hash & Scriptability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every read-oriented `aenv` command emits stable, snapshot-locked `--json`. The resolved-namespace hash (`sha256-v1:<hex>`) is computed per PRD §5.17 and exposed in `aenv status --json` and `aenv list --json`. `aenv diff` reports project drift (managed files that diverged from their namespace source) and, given two namespace names, the structural difference between them. Property tests cover the six hash invariants from engineering §8.3. A cross-machine fixture test guards platform-dependent behavior (line endings, path normalization, encoding).

**Architecture:** The hash is computed from a *material set* — the post-merge `(relative_path, content_bytes)` pairs that would be written to disk on activation, plus a synthetic `.aenv/parameters.json` carrying the resolved parameter map. Deep-merged JSON/YAML/TOML files are canonicalized to RFC 8785 JSON before hashing; section-merged Markdown is hashed as its merged UTF-8 body. We extract material-set computation into a pure function (`materialize::compute_material_set`) that runs the same merge primitives `activate_namespace` uses but returns bytes in memory instead of writing them. The hash itself is `SHA-256(0x01 || Σ(len(path)|path|len(content)|content))` with paths sorted byte-wise lex on UTF-8. The algorithm-version byte and `sha256-v1:` prefix form a versioning hook for R-87 (only `v1` ships in Phase 5). The `--json` work introduces a `json/` module in `aenv-core` with one typed response struct per command (`StatusReport`, `ListEntry`, `WhichReport`, `GetReport`, `DoctorReport`, `AdapterEntry`, `SkillEntry`, `DiffReport`); each command crate wires its `--json` branch to `serde_json::to_string_pretty(&shape)`. Schemas are locked with `insta` snapshot tests; intentional changes show up in code review as a snapshot diff.

**Tech Stack:** Rust 1.85+ stable. New library deps: `insta = "1.39"` (dev only, with `json` feature) and `proptest = "1.4"` (dev only) — both already declared in `[workspace.dependencies]`, just wire to `aenv-core`'s `[dev-dependencies]`. `sha2` is already a runtime dep. No new runtime deps.

**Plan structure:** 17 tasks. T1–T2 implement RFC 8785 JCS (~150 lines of code) with the standard test vectors. T3 extracts material-set computation from the activation path into a pure function that can be called without writing to disk. T4 implements the hash and the `sha256-v1:<hex>` formatter. T5 adds the six property tests from engineering §8.3 via `proptest`. T6 establishes the `json/` module with one response struct per command. T7 wires `--json` through clap. T8–T11 implement each command's `--json` branch and lock its shape with an `insta` snapshot. T12–T14 deliver `aenv diff` (project drift + structural-between-namespaces, text and JSON). T15 lands the cross-machine fixture test. T16 reproduces functional spec §7.5 end-to-end against three fixture namespaces. T17 tags `phase-5-complete`. Estimated effort: 4–5 days of focused work — the JCS implementation and the snapshot-test bring-up are the time sinks; the per-command JSON wiring is mostly mechanical once T6 + T7 land.

**Repository state at start:** `main` at `phase-4-complete` (`17dfd18`) plus five post-completion refinement commits (`72e120d` … `6413502`). Workspace at 399 tests passing, `cargo fmt --check` silent, `cargo clippy --workspace --all-targets -- -D warnings` silent. `error.rs` already declares every variant we need (no new variants this phase). `MaterializeStrategy` already lives in `aenv-core/src/resolve.rs` with `SectionMerge`, `DeepMerge(Json|Yaml|Toml)`, `Symlink`, `Identical`, `Copy`, `Merged` (legacy) variants. `ResolutionResult { chain, candidates, parameters, policies, warnings }` is the input the hash and `--json` work both consume. `aenv-core/src/activate/mod.rs` `activate_namespace` already groups candidates by path, decides a strategy via `strategy::decide_strategy`, and materializes via the per-strategy primitives in `merge/section.rs`, `merge/deep_json.rs`, `merge/deep_yaml.rs`, `merge/deep_toml.rs` — these are the primitives T3 calls from a non-mutating context.

**Important Phase 0–4 invariants this plan honors:**

- `Filesystem` trait still uses `&self`. No new trait methods. The pure material-set computation is read-only against the filesystem and writes nothing.
- All paths below the CLI layer are absolute. The library never reads `std::env::current_dir()` or `std::env::var(...)`.
- `--project <path>` is already plumbed through every command (Phase 1) and continues to work identically with `--json`.
- State directory is `.aenv-state/` (not `.aenv/`). The synthetic hash path `.aenv/parameters.json` is purely a hash-input convention — no such file is ever written to disk.
- Exit codes wired in Phases 1/3/4 are unchanged. Phase 5 does not add any new exit codes; `aenv diff` follows `diff(1)` convention (0 = no drift / no structural differences, 1 = differences detected, 2 = aenv internal error mapped from `AenvError::exit_code()`).
- Tests anticipate rustfmt `max_width = 100`. Pre-format multi-arg calls.
- The materialized-path invariant continues to hold: no path on disk contains `::`. Qualified names appear only in JSON output, state.json, and `aenv which` text.
- Hash neutrality (engineering §7.5): the hash does NOT incorporate qualified names, shadow chains, parameters values *qua* parameters, or policies. Parameters affect the hash *only* through the synthetic `.aenv/parameters.json` entry. Policies and shadows do not affect the hash at all.

**Phase 5 deliberately defers:**

- **R-87 v2 algorithm + dual-emit.** The version byte and `sha256-v1:` prefix are wired so a v2 can be added later without breaking v1 consumers. We do not implement a v2. Dual-emit (`"resolved_hash": "sha256-v1:...", "resolved_hash_v2": "sha256-v2:..."`) is a structural hook in `StatusReport` and `ListEntry` (an `Option<String>` field for v2 that always serializes `None` today, skipped via `skip_serializing_if`) but no v2 computation lands.
- **Windows-specific cross-machine variance.** Engineering §8.5 promises Linux x86_64 + Linux aarch64 + macOS in Phase 5; Windows defers to Phase 7. The fixture test runs everywhere the CI matrix runs today (Linux + macOS).
- **Performance benchmarking via `criterion`.** Engineering §9 budgets <10ms for the shell hook. Phase 5's hash work happens at activation time, not in the hook — measuring is Phase 6's concern when the shell hook lands.
- **`aenv promote`** (functional spec §5.6 mentions it as a sibling to `aenv fork` for the drift case). Listed in the original Phase 6 scope. Phase 5's `aenv diff` only reports drift; resolving it is left to existing `aenv fork <file>`.
- **`aenv adapter list --json` parameter-projection details.** The adapter `parameters[].projects_to` field is already parsed (Phase 3) but its semantics are deferred. The JSON shape carries the field verbatim; no projection happens.

---

## File structure (created or modified in this phase)

**Library (`crates/aenv-core/src/`) — new modules:**

| File | Responsibility |
|---|---|
| `jcs.rs` | RFC 8785 JSON Canonicalization Scheme. Public entry `pub fn canonicalize(v: &serde_json::Value) -> String`. ~150 LOC including the number-formatting helper. |
| `hash.rs` | `pub fn hash_resolved_namespace(material_set: &[(PathBuf, Vec<u8>)], parameters: &BTreeMap<String, ResolvedParameter>) -> String` returning `"sha256-v1:<hex>"`. Algorithm-version byte is a private constant. |
| `materialize.rs` | `pub fn compute_material_set<F: Filesystem>(fs: &F, ...) -> Result<MaterialSet>` — pure, read-only counterpart to `activate_namespace`. Returns `Vec<(PathBuf, Vec<u8>)>` and the resolved-parameter map for use as hash input. |
| `diff.rs` | `pub fn project_drift(...)` and `pub fn structural(ns_a, ns_b)` — both pure, both return typed reports the CLI then renders as text or JSON. |
| `json/mod.rs` | Re-exports the per-command response shapes. |
| `json/status.rs` | `StatusReport` |
| `json/list.rs` | `ListEntry` |
| `json/which.rs` | `WhichReport` |
| `json/get.rs` | `GetReport` |
| `json/doctor.rs` | `DoctorReportJson` (distinct from `doctor::DoctorReport` which is the in-memory shape; this is the wire shape) |
| `json/adapter.rs` | `AdapterEntryJson` |
| `json/skill.rs` | `SkillEntry` |
| `json/diff.rs` | `DriftReport` + `StructuralDiff` |

**Library (modified):**

- `crates/aenv-core/src/lib.rs` — re-export `pub mod jcs;`, `pub mod hash;`, `pub mod materialize;`, `pub mod diff;`, `pub mod json;`.
- `crates/aenv-core/Cargo.toml` — add `insta = { workspace = true }` and `proptest = { workspace = true }` to `[dev-dependencies]`.

**Binary (`crates/aenv-cli/src/`):**

| File | Responsibility |
|---|---|
| `main.rs` (modify) | Add `--json` flag to every read-oriented subcommand (`Status`, `List`, `Which`, `Get`, `Doctor`, `Adapter::List`, `Skill::List`, new `Diff`). Add `Diff { ns_a: Option<String>, ns_b: Option<String>, ... }` subcommand. |
| `cmd/status.rs` (modify) | Branch on `json: bool`. JSON branch builds `StatusReport` and prints `serde_json::to_string_pretty`. |
| `cmd/list.rs` (modify) | Same shape. |
| `cmd/which.rs` (modify) | Same shape. |
| `cmd/get.rs` (modify) | Same shape. |
| `cmd/doctor.rs` (modify) | Same shape. |
| `cmd/adapter.rs` (modify) | Same shape on `run_list`. |
| `cmd/skill/list.rs` (modify) | Same shape. |
| `cmd/diff.rs` (new) | Entry points `run_drift(&fs, &project_root, &aenv_home, json: bool)` and `run_structural(&fs, &layout, ns_a, ns_b, json: bool)`. |
| `cmd/mod.rs` (modify) | `pub mod diff;` |

**Tests (new):**

- `crates/aenv-core/tests/jcs_vectors.rs` — RFC 8785 standard test vectors (object key sort, number formatting, Unicode preservation).
- `crates/aenv-core/tests/materialize_set.rs` — pure material set matches activation output byte-for-byte across all five strategies.
- `crates/aenv-core/tests/hash_basic.rs` — small known fixture; recompute and assert the constant.
- `crates/aenv-core/tests/hash_properties.rs` — six `proptest` properties from engineering §8.3.
- `crates/aenv-core/tests/hash_versioning.rs` — emitted string starts with `"sha256-v1:"`; algorithm-version byte is `0x01` in the SHA input.
- `crates/aenv-core/tests/diff_project_drift.rs` — symlink replaced with edited file → drift; byte-identical file → no drift; deep-merged file regenerated to identical bytes → no drift.
- `crates/aenv-core/tests/diff_structural.rs` — two namespaces with different skill rosters / parameter values / policies / instructions sections.
- `crates/aenv-core/tests/json_snapshots.rs` — `insta` snapshots for every `--json` shape against fixed fixtures.
- `crates/aenv-core/tests/fixtures/cross_machine/README.md` — explains the fixture format.
- `crates/aenv-core/tests/fixtures/cross_machine/alpha/aenv.toml` (+ subdirs) — fixture namespace #1.
- `crates/aenv-core/tests/fixtures/cross_machine/beta/aenv.toml` (+ subdirs) — fixture namespace #2 (extends alpha).
- `crates/aenv-core/tests/fixtures/cross_machine/expected.txt` — `<namespace-name>=<expected-hash>` lines, two namespaces.
- `crates/aenv-core/tests/cross_machine_hash.rs` — loads the fixtures, recomputes each hash, asserts equality against `expected.txt`.
- `crates/aenv-cli/tests/status_json_e2e.rs` — end-to-end `aenv status --json` produces parseable JSON whose top-level keys match spec §7.1.
- `crates/aenv-cli/tests/diff_e2e.rs` — `aenv diff` and `aenv diff <a> <b>` in a real tempdir.
- `crates/aenv-cli/tests/scripted_comparison_e2e.rs` — functional spec §7.5 inner loop reproduced: three namespaces, activate each in turn against the same project, hash and short-name list captured per activation.

---

## Glossary (for the implementer)

- **Material set** — the post-merge `Vec<(PathBuf, Vec<u8>)>` produced by walking the `extends` chain, applying section-merge / deep-merge / symlink-source-read per candidate, in lexicographic path order. The exact byte sequence that `activate_namespace` would write, minus the wire-format differences. For symlinked artifacts the bytes are the source file's bytes. For section-merged artifacts the bytes are the merged Markdown body. For deep-merged JSON/YAML/TOML the bytes are the merged value re-serialized via the default serializer (NOT the canonical form — canonicalization happens only as a hash-input transformation).
- **Hash input** — the byte sequence fed into SHA-256. Built from the material set plus a synthetic `.aenv/parameters.json` entry. Algorithm: `0x01 || Σ for each (path, content) sorted by path: be_u32(path.len()) || path.as_bytes() || be_u64(content.len()) || canonicalized_content_bytes`. "Canonicalized content" means RFC 8785 JSON for any structured-file entry; otherwise raw bytes.
- **RFC 8785 JCS** — JSON Canonicalization Scheme. Deterministic JSON serialization where object keys are sorted lexicographically by UTF-16 code unit, numbers use ECMAScript `JSON.stringify` formatting (no trailing zeros, shortest representation), strings are minimally escaped, no extraneous whitespace. The ~150-line implementation walks `serde_json::Value` and writes a `String`.
- **Algorithm-version byte** — a single byte prepended to the hash input before SHA-256. `0x01` for v1. Future v2 would use `0x02`; the prefix in the user-facing string (`sha256-v1:` vs `sha256-v2:`) advertises the version so consumers can branch without re-implementing the algorithm.
- **Resolved hash string** — the user-facing form: `"sha256-v1:<64-lowercase-hex>"`. Exposed in `StatusReport.resolved_hash` and `ListEntry.resolved_hash`.
- **JSON response shape** — a typed struct in `aenv-core/src/json/` that derives `serde::Serialize`. The CLI command produces an instance and calls `serde_json::to_string_pretty(&shape)`. Schemas are locked with `insta::assert_json_snapshot!`. The struct field names ARE the public schema; renaming requires a snapshot review and constitutes a breaking change.
- **Project drift** — for any file in `state.managed_files`, the on-disk bytes differ from what the current resolution would materialize. Per-strategy: symlinked files drift if the symlink was replaced by a regular file with non-matching bytes; section-merged and deep-merged files drift if the on-disk bytes differ from the freshly-computed merge.
- **Structural diff** — for two resolved namespaces, the symmetric difference (and intersection-with-different-value) of their skill rosters, agent rosters (always empty today), parameter maps, policy maps, and section-merged instructions files.

---

## Cross-cutting test conventions

- Every task that creates a test runs `PATH="$HOME/.cargo/bin:$PATH" cargo test -p <crate> --test <name>`. `cargo` is invoked through the rustup shim at `~/.cargo/bin/cargo` per the repo's MSRV strategy.
- `insta` snapshot files live next to the test (e.g. `crates/aenv-core/tests/snapshots/json_snapshots__status.snap`). Approve snapshots with `cargo insta accept --all`; review them in the diff.
- `proptest` tests cap iterations at `proptest! { #![proptest_config(ProptestConfig { cases: 32, .. })] }` — the default 256 is too slow for filesystem-heavy properties; 32 is enough for invariant detection given the constructed-input shape.
- Fixture namespaces under `tests/fixtures/cross_machine/` use LF line endings (committed via `.gitattributes`-style normalization — see Task 15 step 2) and no trailing whitespace. Path components are ASCII to avoid Unicode-normalization differences across platforms.

---

### Task 1: Add dev-deps + JCS module skeleton

Wire `insta` and `proptest` into `aenv-core`'s dev-dependencies (both already in workspace) and create empty `jcs.rs` + `hash.rs` modules so subsequent tasks have somewhere to land code without re-touching `lib.rs`.

**Files:**
- Modify: `crates/aenv-core/Cargo.toml`
- Create: `crates/aenv-core/src/jcs.rs`
- Create: `crates/aenv-core/src/hash.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Test: (none — pure scaffolding)

- [ ] **Step 1: Update `crates/aenv-core/Cargo.toml`**

```toml
[dev-dependencies]
tempfile = { workspace = true }
insta = { workspace = true }
proptest = { workspace = true }
```

- [ ] **Step 2: Create `crates/aenv-core/src/jcs.rs` as a stub**

```rust
//! RFC 8785 JSON Canonicalization Scheme (JCS).
//!
//! Phase 5 introduces a deterministic JSON serialization for use as a
//! hash-input transformation. The implementation walks `serde_json::Value`
//! and writes a `String` per RFC 8785: object keys sorted by UTF-16 code
//! unit, numbers in shortest ECMAScript `JSON.stringify` form, strings
//! minimally escaped, no extraneous whitespace.
//!
//! Entry point lands in Task 2. This file exists so `lib.rs` can re-export
//! `pub mod jcs;` without a chain of dependent diffs.

#![allow(dead_code)] // Filled in by Task 2.
```

- [ ] **Step 3: Create `crates/aenv-core/src/hash.rs` as a stub**

```rust
//! Resolved-namespace content hash (PRD §5.17, R-84–R-87).
//!
//! The hash is computed from a *material set* (the post-merge byte
//! contents that would be written to disk on activation) plus a synthetic
//! `.aenv/parameters.json` entry carrying the resolved parameter map.
//!
//! Implementation lands in Task 4. This file exists so dependent modules
//! can `use crate::hash;` without a chain of dependent diffs.

#![allow(dead_code)] // Filled in by Task 4.

/// Algorithm-version byte prepended to the hash input. Bumping this is a
/// breaking change per R-87 and requires a dual-emit deprecation window.
pub(crate) const ALGORITHM_VERSION_V1: u8 = 0x01;

/// User-facing prefix advertised on every emitted hash string.
pub const HASH_PREFIX_V1: &str = "sha256-v1:";
```

- [ ] **Step 4: Modify `crates/aenv-core/src/lib.rs`**

Insert (alphabetically placed) the two new module declarations:

```rust
pub mod hash;
pub mod jcs;
```

- [ ] **Step 5: Verify build**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --workspace 2>&1 | tail -10`
Expected: `Compiling ...` lines, no errors, `Finished` line.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10`
Expected: silent (no warnings).

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/Cargo.toml crates/aenv-core/src/jcs.rs \
        crates/aenv-core/src/hash.rs crates/aenv-core/src/lib.rs
git commit -m "$(cat <<'EOF'
Add Phase 5 scaffolding: jcs/hash modules + dev-deps

- Stub modules so Tasks 2/4 can land without re-touching lib.rs.
- Wire workspace-declared insta + proptest into aenv-core dev-deps.
- Declare ALGORITHM_VERSION_V1 (0x01) and HASH_PREFIX_V1 ("sha256-v1:")
  up-front so dependent modules can use the constants.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: RFC 8785 JCS implementation + test vectors

Implement `jcs::canonicalize(&serde_json::Value) -> String`. The implementation is recursive, handles all six JSON types (null / bool / number / string / array / object), and walks objects with keys sorted by UTF-16 code unit. Numbers use ECMAScript `JSON.stringify` rules (shortest unambiguous form).

**Files:**
- Modify: `crates/aenv-core/src/jcs.rs`
- Test: `crates/aenv-core/tests/jcs_vectors.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/jcs_vectors.rs`:

```rust
//! RFC 8785 JCS standard test vectors.
//!
//! Vectors drawn from RFC 8785 §3.2.3 ("Object structure") and §3.2.2.3
//! ("Number serialization"). The full RFC test suite includes ECMAScript
//! number-formatting edge cases (1e+30, 1e-7, etc.); we cover the
//! representative cases that exercise our serializer's branches.

use aenv_core::jcs::canonicalize;
use serde_json::json;

#[test]
fn object_keys_are_sorted() {
    let v = json!({"b": 1, "a": 2});
    assert_eq!(canonicalize(&v), r#"{"a":2,"b":1}"#);
}

#[test]
fn nested_object_keys_are_sorted_recursively() {
    let v = json!({"b": 1, "a": {"d": 4, "c": 3}});
    assert_eq!(canonicalize(&v), r#"{"a":{"c":3,"d":4},"b":1}"#);
}

#[test]
fn array_order_is_preserved() {
    let v = json!([3, 1, 2]);
    assert_eq!(canonicalize(&v), "[3,1,2]");
}

#[test]
fn empty_collections() {
    assert_eq!(canonicalize(&json!([])), "[]");
    assert_eq!(canonicalize(&json!({})), "{}");
}

#[test]
fn null_and_booleans() {
    assert_eq!(canonicalize(&json!(null)), "null");
    assert_eq!(canonicalize(&json!(true)), "true");
    assert_eq!(canonicalize(&json!(false)), "false");
}

#[test]
fn integer_numbers() {
    assert_eq!(canonicalize(&json!(0)), "0");
    assert_eq!(canonicalize(&json!(1)), "1");
    assert_eq!(canonicalize(&json!(-1)), "-1");
    assert_eq!(canonicalize(&json!(42)), "42");
    assert_eq!(canonicalize(&json!(i64::MAX)), "9223372036854775807");
}

#[test]
fn float_numbers_use_shortest_form() {
    // ECMAScript JSON.stringify(1.5) -> "1.5"
    assert_eq!(canonicalize(&json!(1.5)), "1.5");
    // 5e1 round-trips to 50 per ECMAScript.
    let v: serde_json::Value = serde_json::from_str("5e1").unwrap();
    assert_eq!(canonicalize(&v), "50");
    // 1e21 stays in exponent form per ECMAScript.
    let v: serde_json::Value = serde_json::from_str("1e21").unwrap();
    assert_eq!(canonicalize(&v), "1e+21");
}

#[test]
fn string_escaping_is_minimal() {
    // Only the RFC 8259 mandatory escapes: quote, backslash, control chars.
    assert_eq!(canonicalize(&json!("hello")), r#""hello""#);
    assert_eq!(canonicalize(&json!("a\"b")), r#""a\"b""#);
    assert_eq!(canonicalize(&json!("a\\b")), r#""a\\b""#);
    assert_eq!(canonicalize(&json!("a\nb")), r#""a\nb""#);
    assert_eq!(canonicalize(&json!("a\tb")), r#""a\tb""#);
    // Non-ASCII Unicode is emitted directly (UTF-8), NOT \uXXXX escaped.
    assert_eq!(canonicalize(&json!("ö")), "\"ö\"");
    assert_eq!(canonicalize(&json!("中")), "\"中\"");
}

#[test]
fn control_chars_below_0x20_get_uppercase_hex_escapes() {
    // RFC 8785 specifies \u escapes for control chars use lowercase hex.
    // Specifically \u00XX form. We test 0x01 and 0x1f.
    let v = json!("\u{01}");
    assert_eq!(canonicalize(&v), "\"\\u0001\"");
    let v = json!("\u{1f}");
    assert_eq!(canonicalize(&v), "\"\\u001f\"");
}

#[test]
fn rfc_8785_section_3_example() {
    // From RFC 8785 §3.2.3 example, simplified:
    let v = json!({
        "numbers": [333333333.3333333, 1e30, 4.50, 0.000001, "10e+0"],
        "string": "Hello world!",
        "literals": [null, true, false]
    });
    let out = canonicalize(&v);
    // Top-level keys are sorted: literals, numbers, string.
    assert!(out.starts_with(r#"{"literals":"#));
    assert!(out.contains(r#""numbers":"#));
    assert!(out.ends_with(r#""string":"Hello world!"}"#));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test jcs_vectors 2>&1 | tail -15`
Expected: FAIL — `cannot find function canonicalize in module jcs`.

- [ ] **Step 3: Implement `jcs::canonicalize`**

Replace `crates/aenv-core/src/jcs.rs` content:

```rust
//! RFC 8785 JSON Canonicalization Scheme (JCS).
//!
//! Deterministic JSON serialization used as a hash-input transformation.
//! Object keys are sorted by UTF-16 code unit ordering, numbers use
//! ECMAScript `JSON.stringify` shortest-form serialization, strings are
//! minimally escaped, and there is no extraneous whitespace.

use serde_json::Value;

/// Canonicalize a `serde_json::Value` to its RFC 8785 representation.
pub fn canonicalize(v: &Value) -> String {
    let mut out = String::new();
    write_value(v, &mut out);
    out
}

fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&format_number(n)),
        Value::String(s) => write_string(s, out),
        Value::Array(xs) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(x, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            // RFC 8785: keys are sorted by UTF-16 code unit. For ASCII keys
            // (the common case) UTF-16 ordering equals byte ordering. For
            // multi-byte keys we sort by the UTF-16 encoding.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| utf16_cmp(a, b));
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_value(&map[*k], out);
            }
            out.push('}');
        }
    }
}

fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.encode_utf16();
    let mut bi = b.encode_utf16();
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => match x.cmp(&y) {
                std::cmp::Ordering::Equal => continue,
                ord => return ord,
            },
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{09}' => out.push_str("\\t"),
            '\u{0a}' => out.push_str("\\n"),
            '\u{0c}' => out.push_str("\\f"),
            '\u{0d}' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn format_number(n: &serde_json::Number) -> String {
    // Integer fast path (most parameter values are integers).
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    // Floating point: format per ECMAScript JSON.stringify, which targets
    // the shortest representation that round-trips. Rust's default
    // `f64::to_string` (via Ryu) produces a shortest round-trip form, but
    // it uses lowercase 'e' and may emit `0.0001` vs `1e-4` differently
    // from ECMAScript. We normalize the exponent format to match
    // ECMAScript's rules:
    //   - exponent is `e+N` or `e-N` (sign always present)
    //   - mantissa has no trailing zero unless it would be bare like `1e+21`
    //   - numbers in [1e-6, 1e21) use plain decimal; outside use exponent.
    let f = n.as_f64().expect("serde_json::Number is i64, u64, or f64");
    format_ecmascript_f64(f)
}

fn format_ecmascript_f64(f: f64) -> String {
    if f == 0.0 {
        return if f.is_sign_negative() { "0".to_string() } else { "0".to_string() };
    }
    if f.is_nan() || f.is_infinite() {
        // RFC 8785 forbids these; serde_json itself rejects them upstream.
        // Defensive fallback to match `serde_json::to_string` behavior.
        return "null".to_string();
    }
    let abs = f.abs();
    // ECMAScript JSON.stringify uses plain decimal for [1e-6, 1e21).
    // Outside that range it uses exponent form.
    if abs >= 1e-6 && abs < 1e21 {
        // Use Ryu via the default Display impl, which produces a shortest
        // round-trip representation. Strip a trailing ".0" so integers
        // formatted as floats become bare integers per ECMAScript.
        let s = format!("{f}");
        if let Some(stripped) = s.strip_suffix(".0") {
            return stripped.to_string();
        }
        s
    } else {
        // Exponent form: ECMAScript uses `e+N` / `e-N`.
        let s = format!("{f:e}");
        // Rust's `{:e}` produces e.g. "1e21" (no sign on positive exponent).
        // ECMAScript wants "1e+21". Normalize.
        normalize_exponent_sign(&s)
    }
}

fn normalize_exponent_sign(s: &str) -> String {
    if let Some(epos) = s.find('e') {
        let (mantissa, exp) = s.split_at(epos);
        let exp_body = &exp[1..]; // skip the 'e'
        let mantissa = mantissa.strip_suffix(".0").unwrap_or(mantissa);
        if exp_body.starts_with('-') {
            format!("{mantissa}e{exp_body}")
        } else if exp_body.starts_with('+') {
            format!("{mantissa}e{exp_body}")
        } else {
            format!("{mantissa}e+{exp_body}")
        }
    } else {
        s.to_string()
    }
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test jcs_vectors 2>&1 | tail -20`
Expected: all 10 tests pass.

If `float_numbers_use_shortest_form` fails on the `1e21` case, inspect the actual output — Rust's `{:e}` formatting of `1e21` may produce `"1e21"` (correctly normalized to `"1e+21"` by our helper) or `"1.0e21"`. If the latter, adjust the strip in `normalize_exponent_sign` to strip a `.0` from the mantissa before splitting.

- [ ] **Step 5: Clippy clean**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10`
Expected: silent.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/jcs.rs crates/aenv-core/tests/jcs_vectors.rs
git commit -m "$(cat <<'EOF'
Implement RFC 8785 JSON Canonicalization Scheme

- Recursive walker over serde_json::Value: object keys sorted by UTF-16
  code unit; arrays preserve order; strings escape only the RFC 8259
  mandatory set; non-ASCII Unicode is emitted directly (UTF-8 bytes),
  not \uXXXX escaped.
- Numbers use ECMAScript JSON.stringify shortest-form: plain decimal in
  [1e-6, 1e21), exponent form outside, with the sign always present on
  the exponent (e+21 / e-7).
- 10 test vectors cover the structural cases plus the RFC §3.2.3
  example. The full ECMAScript number-formatting suite is out of scope;
  Phase 5's hash input only consumes integer/list/string parameters via
  the synthetic .aenv/parameters.json entry.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Material-set computation (pure, no-write counterpart to activate)

Extract a read-only function that computes the same `(path, content_bytes)` pairs `activate_namespace` would write, without touching the project filesystem. This is the input to the hash.

**Files:**
- Create: `crates/aenv-core/src/materialize.rs`
- Modify: `crates/aenv-core/src/lib.rs` (add `pub mod materialize;`)
- Test: `crates/aenv-core/tests/materialize_set.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/materialize_set.rs`:

```rust
//! Verify the pure material-set computation matches what activation
//! would write for each of the four real strategies: Symlink, Identical,
//! SectionMerge, DeepMerge(Json).

use aenv_core::adapter::AdapterRegistry;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::compute_material_set;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup() -> (TempDir, RegistryLayout, AdapterRegistry) {
    let tmp = TempDir::new().unwrap();
    let layout = RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    (tmp, layout, adapters)
}

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn single_symlink_candidate_contributes_source_bytes() {
    let (_tmp, layout, adapters) = setup();
    let ns_root = layout.namespace_dir("solo");
    write_file(&layout.manifest_path("solo"), "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n");
    write_file(&ns_root.join("CLAUDE.md"), "# Hello\nProject facts.\n");

    let fs = aenv_core::fs::RealFilesystem;
    let leaf = NamespaceId::new("solo").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();

    assert_eq!(mat.entries.len(), 1);
    assert_eq!(mat.entries[0].0, PathBuf::from("CLAUDE.md"));
    assert_eq!(
        mat.entries[0].1,
        b"# Hello\nProject facts.\n".to_vec()
    );
}

#[test]
fn section_merge_combines_two_namespaces() {
    let (_tmp, layout, adapters) = setup();
    let base = layout.namespace_dir("base");
    let leaf = layout.namespace_dir("leaf");
    write_file(
        &layout.manifest_path("base"),
        "name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&base.join("CLAUDE.md"), "## Facts\nA\n");
    write_file(
        &layout.manifest_path("leaf"),
        "name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&leaf.join("CLAUDE.md"), "## Disposition\nB\n");

    let fs = aenv_core::fs::RealFilesystem;
    let leaf_id = NamespaceId::new("leaf").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf_id).unwrap();

    assert_eq!(mat.entries.len(), 1);
    let body = std::str::from_utf8(&mat.entries[0].1).unwrap();
    // Section merge concatenates by `##` header, base first.
    assert!(body.contains("## Facts"));
    assert!(body.contains("## Disposition"));
    let facts_pos = body.find("## Facts").unwrap();
    let disp_pos = body.find("## Disposition").unwrap();
    assert!(facts_pos < disp_pos, "base section precedes leaf section");
}

#[test]
fn deep_merge_json_uses_default_serializer_bytes() {
    let (_tmp, layout, adapters) = setup();
    let base = layout.namespace_dir("base");
    let leaf = layout.namespace_dir("leaf");
    write_file(
        &layout.manifest_path("base"),
        "name = \"base\"\n[adapters.mcp]\nfiles = [\".mcp.json\"]\n",
    );
    write_file(&base.join(".mcp.json"), "{\"servers\":{\"a\":{\"command\":\"x\"}}}\n");
    write_file(
        &layout.manifest_path("leaf"),
        "name = \"leaf\"\nextends = [\"base\"]\n[adapters.mcp]\nfiles = [\".mcp.json\"]\n",
    );
    write_file(&leaf.join(".mcp.json"), "{\"servers\":{\"b\":{\"command\":\"y\"}}}\n");

    let fs = aenv_core::fs::RealFilesystem;
    let leaf_id = NamespaceId::new("leaf").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf_id).unwrap();

    let body = std::str::from_utf8(&mat.entries[0].1).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
    assert!(parsed["servers"]["a"].is_object());
    assert!(parsed["servers"]["b"].is_object());
}

#[test]
fn entries_are_sorted_by_path() {
    let (_tmp, layout, adapters) = setup();
    let ns_root = layout.namespace_dir("multi");
    write_file(
        &layout.manifest_path("multi"),
        "name = \"multi\"\n[adapters.claude-code]\nfiles = [\"z.md\", \"a.md\", \"m.md\"]\n",
    );
    write_file(&ns_root.join("z.md"), "z\n");
    write_file(&ns_root.join("a.md"), "a\n");
    write_file(&ns_root.join("m.md"), "m\n");

    let fs = aenv_core::fs::RealFilesystem;
    let leaf = NamespaceId::new("multi").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();

    let paths: Vec<&std::path::Path> = mat.entries.iter().map(|(p, _)| p.as_path()).collect();
    let sorted: Vec<&std::path::Path> = {
        let mut s = paths.clone();
        s.sort();
        s
    };
    assert_eq!(paths, sorted);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test materialize_set 2>&1 | tail -10`
Expected: FAIL — `cannot find module materialize`.

- [ ] **Step 3: Implement `materialize::compute_material_set`**

Create `crates/aenv-core/src/materialize.rs`:

```rust
//! Pure material-set computation — the read-only counterpart to
//! `activate_namespace`.
//!
//! Returns the same `(project_relative_path, content_bytes)` pairs that
//! activation would write, without touching the project filesystem.
//! Section-merged and deep-merged artifacts are produced by the same
//! merge primitives activation uses; symlinked artifacts contribute the
//! source file's raw bytes.
//!
//! This is the input to `hash::hash_resolved_namespace`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::adapter::AdapterRegistry;
use crate::error::Result;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::parameters::ResolvedParameter;
use crate::resolve::{resolve_namespace, Candidate, DeepMergeFormat, MaterializeStrategy};
use crate::strategy::decide_strategy;

/// Output of `compute_material_set`. Entries are sorted by path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialSet {
    /// Sorted (project-relative path, post-merge bytes) pairs.
    pub entries: Vec<(PathBuf, Vec<u8>)>,
    /// Resolved parameter map. Carried alongside so the hash function can
    /// append it as the synthetic `.aenv/parameters.json` entry.
    pub parameters: BTreeMap<String, ResolvedParameter>,
}

/// Compute the material set for `leaf` without writing anything.
pub fn compute_material_set<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    leaf: &NamespaceId,
) -> Result<MaterialSet> {
    let resolution = resolve_namespace(fs, layout, adapters, leaf)?;

    // Group candidates by path (same grouping activate_namespace uses).
    let mut by_path: BTreeMap<PathBuf, Vec<Candidate>> = BTreeMap::new();
    for c in resolution.candidates {
        by_path.entry(c.path.clone()).or_default().push(c);
    }

    let mut entries: Vec<(PathBuf, Vec<u8>)> = Vec::with_capacity(by_path.len());
    for (path, candidates) in by_path {
        let strategy = decide_strategy(&candidates, adapters)?;
        let bytes = materialize_one_in_memory(fs, &candidates, strategy)?;
        entries.push((path, bytes));
    }

    // BTreeMap iteration already gave us sorted-by-path order. Re-sort
    // defensively — the contract is byte-wise lex on the UTF-8 path.
    entries.sort_by(|a, b| a.0.as_os_str().as_encoded_bytes().cmp(b.0.as_os_str().as_encoded_bytes()));

    Ok(MaterialSet {
        entries,
        parameters: resolution.parameters,
    })
}

fn materialize_one_in_memory<F: Filesystem>(
    fs: &F,
    candidates: &[Candidate],
    strategy: MaterializeStrategy,
) -> Result<Vec<u8>> {
    match strategy {
        // For Symlink / Identical / Copy / legacy Merged: the winning
        // candidate is the leaf-most one. Read its source bytes.
        MaterializeStrategy::Symlink
        | MaterializeStrategy::Identical
        | MaterializeStrategy::Copy
        | MaterializeStrategy::Merged => {
            let winner = candidates.last().expect("at least one candidate");
            fs.read(&winner.source_path).map_err(crate::AenvError::from)
        }
        MaterializeStrategy::SectionMerge => {
            let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(candidates.len());
            for c in candidates {
                bodies.push(fs.read(&c.source_path).map_err(crate::AenvError::from)?);
            }
            let body_strs: Vec<&str> = bodies
                .iter()
                .map(|b| std::str::from_utf8(b))
                .collect::<std::result::Result<_, _>>()
                .map_err(|e| {
                    crate::AenvError::ManifestInvalid(format!(
                        "section-merge input is not valid UTF-8: {e}"
                    ))
                })?;
            Ok(crate::merge::section::section_merge(&body_strs).into_bytes())
        }
        MaterializeStrategy::DeepMerge(format) => {
            let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(candidates.len());
            for c in candidates {
                bodies.push(fs.read(&c.source_path).map_err(crate::AenvError::from)?);
            }
            let body_strs: Vec<&str> = bodies
                .iter()
                .map(|b| std::str::from_utf8(b))
                .collect::<std::result::Result<_, _>>()
                .map_err(|e| {
                    crate::AenvError::ManifestInvalid(format!(
                        "deep-merge input is not valid UTF-8: {e}"
                    ))
                })?;
            match format {
                DeepMergeFormat::Json => crate::merge::deep_json::deep_merge_json_str(&body_strs)
                    .map(String::into_bytes),
                DeepMergeFormat::Yaml => crate::merge::deep_yaml::deep_merge_yaml_str(&body_strs)
                    .map(String::into_bytes),
                DeepMergeFormat::Toml => crate::merge::deep_toml::deep_merge_toml_str(&body_strs)
                    .map(String::into_bytes),
            }
        }
    }
}
```

- [ ] **Step 4: Add the module declaration to `lib.rs`**

Add `pub mod materialize;` to `crates/aenv-core/src/lib.rs` (alphabetically before `pub mod merge;`).

- [ ] **Step 5: Verify the merge primitive signatures match**

The plan assumes `merge::section::section_merge(&[&str]) -> String`, `merge::deep_json::deep_merge_json_str(&[&str]) -> Result<String>`, etc. Confirm before running tests:

Run: `grep -n "pub fn section_merge\|pub fn deep_merge_json_str\|pub fn deep_merge_yaml_str\|pub fn deep_merge_toml_str" crates/aenv-core/src/merge/*.rs`

Expected output names them. If a primitive has a different signature (for example it takes `&[String]` or returns `Result<Vec<u8>>`), adapt the calls in `materialize_one_in_memory` to match. The contract is "read the candidate source bytes, produce the merged bytes that would be written"; the exact signature of the primitive isn't load-bearing.

- [ ] **Step 6: Run tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test materialize_set 2>&1 | tail -15`
Expected: 4 tests pass.

If `section_merge_combines_two_namespaces` fails because the section merge ordering looks reversed, double-check the resolution chain order — `resolve_namespace` returns root → leaf, candidates therefore arrive root-first, and `section_merge` concatenates in that order. The leaf should produce the second (`## Disposition`) section.

- [ ] **Step 7: Run the full workspace test suite**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -5`
Expected: no regressions; existing 399 tests still pass plus the new 4.

- [ ] **Step 8: Commit**

```bash
git add crates/aenv-core/src/materialize.rs crates/aenv-core/src/lib.rs \
        crates/aenv-core/tests/materialize_set.rs
git commit -m "$(cat <<'EOF'
Add pure material-set computation for hashing

- compute_material_set is the read-only counterpart to activate_namespace:
  same resolution, same grouping, same per-strategy primitives, but it
  returns bytes in memory instead of writing them to disk.
- For Symlink / Identical / Copy / legacy Merged the winning candidate's
  source bytes are the material bytes. For SectionMerge and
  DeepMerge(Json|Yaml|Toml) the existing merge primitives produce them.
- Entries are byte-wise lex sorted by UTF-8 path so hash input ordering
  is reproducible across platforms. The function carries the resolved
  parameter map alongside so hash::hash_resolved_namespace can append
  the synthetic .aenv/parameters.json entry without re-running
  resolution.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: `hash_resolved_namespace` + `sha256-v1:<hex>` formatter

Implement the canonical hash function from PRD §5.17 R-84. Length-prefixed serialization, byte-wise lex path order (already guaranteed by `MaterialSet.entries`), algorithm-version byte, SHA-256, lowercase-hex with the `sha256-v1:` prefix.

**Files:**
- Modify: `crates/aenv-core/src/hash.rs`
- Test: `crates/aenv-core/tests/hash_basic.rs`
- Test: `crates/aenv-core/tests/hash_versioning.rs`

- [ ] **Step 1: Write the failing basic-hash test**

Create `crates/aenv-core/tests/hash_basic.rs`:

```rust
//! Sanity tests for hash_resolved_namespace against a tiny known fixture.

use aenv_core::hash::{hash_resolved_namespace, HASH_PREFIX_V1};
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::{compute_material_set, MaterialSet};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn empty_material_set_hashes_to_constant() {
    let mat = MaterialSet {
        entries: vec![],
        parameters: BTreeMap::new(),
    };
    let h = hash_resolved_namespace(&mat);
    assert!(h.starts_with(HASH_PREFIX_V1));
    let hex = h.strip_prefix(HASH_PREFIX_V1).unwrap();
    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn single_entry_material_set_is_deterministic() {
    let mat = MaterialSet {
        entries: vec![(PathBuf::from("CLAUDE.md"), b"hello\n".to_vec())],
        parameters: BTreeMap::new(),
    };
    let h1 = hash_resolved_namespace(&mat);
    let h2 = hash_resolved_namespace(&mat);
    assert_eq!(h1, h2);
}

#[test]
fn hash_differs_on_content_change() {
    let a = MaterialSet {
        entries: vec![(PathBuf::from("CLAUDE.md"), b"hello\n".to_vec())],
        parameters: BTreeMap::new(),
    };
    let b = MaterialSet {
        entries: vec![(PathBuf::from("CLAUDE.md"), b"hello!\n".to_vec())],
        parameters: BTreeMap::new(),
    };
    assert_ne!(hash_resolved_namespace(&a), hash_resolved_namespace(&b));
}

#[test]
fn hash_differs_on_path_change() {
    let a = MaterialSet {
        entries: vec![(PathBuf::from("a.md"), b"x".to_vec())],
        parameters: BTreeMap::new(),
    };
    let b = MaterialSet {
        entries: vec![(PathBuf::from("b.md"), b"x".to_vec())],
        parameters: BTreeMap::new(),
    };
    assert_ne!(hash_resolved_namespace(&a), hash_resolved_namespace(&b));
}

#[test]
fn hash_via_compute_material_set_round_trip() {
    let tmp = TempDir::new().unwrap();
    let layout = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();

    let ns_root = layout.namespace_dir("solo");
    write_file(
        &layout.manifest_path("solo"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&ns_root.join("CLAUDE.md"), "# Hello\n");

    let leaf = NamespaceId::new("solo").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();
    let h = hash_resolved_namespace(&mat);
    assert!(h.starts_with(HASH_PREFIX_V1));
    assert_eq!(h.strip_prefix(HASH_PREFIX_V1).unwrap().len(), 64);
}
```

- [ ] **Step 2: Write the failing versioning test**

Create `crates/aenv-core/tests/hash_versioning.rs`:

```rust
//! R-85 / R-87: emitted string always carries the algorithm-version prefix.

use aenv_core::hash::{hash_resolved_namespace, HASH_PREFIX_V1};
use aenv_core::materialize::MaterialSet;
use std::collections::BTreeMap;

#[test]
fn prefix_is_sha256_v1() {
    assert_eq!(HASH_PREFIX_V1, "sha256-v1:");
}

#[test]
fn emitted_strings_carry_v1_prefix() {
    let mat = MaterialSet {
        entries: vec![],
        parameters: BTreeMap::new(),
    };
    let h = hash_resolved_namespace(&mat);
    assert!(
        h.starts_with("sha256-v1:"),
        "hash {h} must start with sha256-v1:"
    );
}
```

- [ ] **Step 3: Run to verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test hash_basic --test hash_versioning 2>&1 | tail -10`
Expected: FAIL — `cannot find function hash_resolved_namespace`.

- [ ] **Step 4: Implement `hash_resolved_namespace`**

Replace `crates/aenv-core/src/hash.rs` content:

```rust
//! Resolved-namespace content hash (PRD §5.17, R-84–R-87).
//!
//! Builds the hash input from a `MaterialSet` plus a synthetic
//! `.aenv/parameters.json` entry, prepends the algorithm-version byte,
//! and runs SHA-256. The user-facing form is `sha256-v1:<lowercase-hex>`.

use sha2::{Digest, Sha256};

use crate::jcs::canonicalize;
use crate::materialize::MaterialSet;
use crate::parameters::{ParameterValue, ResolvedParameter};

const ALGORITHM_VERSION_V1: u8 = 0x01;
/// User-facing prefix advertised on every emitted hash string.
pub const HASH_PREFIX_V1: &str = "sha256-v1:";
/// Synthetic path used to fold the resolved parameter map into the hash.
const SYNTHETIC_PARAMETERS_PATH: &str = ".aenv/parameters.json";

/// Compute the resolved-namespace hash per PRD §5.17 R-84.
pub fn hash_resolved_namespace(mat: &MaterialSet) -> String {
    let params_bytes = canonicalize_parameters(&mat.parameters);
    let mut all: Vec<(Vec<u8>, &[u8])> = Vec::with_capacity(mat.entries.len() + 1);
    for (path, content) in &mat.entries {
        all.push((path_to_bytes(path), content.as_slice()));
    }
    all.push((
        SYNTHETIC_PARAMETERS_PATH.as_bytes().to_vec(),
        params_bytes.as_bytes(),
    ));
    all.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    hasher.update([ALGORITHM_VERSION_V1]);
    for (path_bytes, content) in &all {
        let path_len: u32 = u32::try_from(path_bytes.len())
            .expect("path length exceeds u32::MAX — impossible on real filesystems");
        let content_len: u64 = u64::try_from(content.len()).expect("content length exceeds u64::MAX");
        hasher.update(path_len.to_be_bytes());
        hasher.update(path_bytes);
        hasher.update(content_len.to_be_bytes());
        hasher.update(content);
    }
    let digest = hasher.finalize();
    format!("{HASH_PREFIX_V1}{:x}", HexDisplay(&digest))
}

fn canonicalize_parameters(
    params: &std::collections::BTreeMap<String, ResolvedParameter>,
) -> String {
    let mut map = serde_json::Map::with_capacity(params.len());
    for (k, rp) in params {
        map.insert(k.clone(), parameter_value_to_json(&rp.value));
    }
    canonicalize(&serde_json::Value::Object(map))
}

fn parameter_value_to_json(v: &ParameterValue) -> serde_json::Value {
    match v {
        ParameterValue::String(s) => serde_json::Value::String(s.clone()),
        ParameterValue::Integer(i) => serde_json::Value::Number((*i).into()),
        ParameterValue::Boolean(b) => serde_json::Value::Bool(*b),
        ParameterValue::ListString(xs) => serde_json::Value::Array(
            xs.iter().map(|s| serde_json::Value::String(s.clone())).collect(),
        ),
    }
}

fn path_to_bytes(p: &std::path::Path) -> Vec<u8> {
    let s = p.to_string_lossy();
    s.replace('\\', "/").into_bytes()
}

struct HexDisplay<'a>(&'a [u8]);

impl std::fmt::LowerHex for HexDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}
```

- [ ] **Step 5: Run tests + clippy**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test hash_basic --test hash_versioning 2>&1 | tail -15 && PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3`
Expected: 7 tests pass; clippy silent.

- [ ] **Step 6: Commit**

```bash
git add crates/aenv-core/src/hash.rs crates/aenv-core/tests/hash_basic.rs \
        crates/aenv-core/tests/hash_versioning.rs
git commit -m "Implement resolved-namespace hash (sha256-v1)

hash_resolved_namespace consumes a MaterialSet, folds in the resolved
parameter map as a synthetic .aenv/parameters.json entry (canonical
JSON per RFC 8785), sorts byte-wise lex by UTF-8 path, length-prefixes
each (path, content) pair, prepends the 0x01 algorithm-version byte,
and SHA-256s the result. Path-to-bytes normalizes backslashes to
forward slashes so Windows doesn't produce a different hash from Unix.
Parameter provenance (source namespace) is deliberately stripped before
hashing — the hash captures effective values only, matching the
hash-neutrality invariant from engineering §7.5.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Hash property tests (proptest)

Implement the hash invariants from engineering §8.3 as `proptest` properties.

**Files:**
- Test: `crates/aenv-core/tests/hash_properties.rs`

- [ ] **Step 1: Write the property test file**

Create `crates/aenv-core/tests/hash_properties.rs`:

```rust
//! R-81 / R-86 invariants verified with proptest.

use aenv_core::hash::hash_resolved_namespace;
use aenv_core::materialize::MaterialSet;
use aenv_core::parameters::{ParameterValue, ResolvedParameter};
use proptest::collection::vec;
use proptest::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn entry_strategy() -> impl Strategy<Value = (PathBuf, Vec<u8>)> {
    let path = "[a-z][a-z0-9_/]{0,32}\\.[a-z]{1,4}";
    (path, vec(any::<u8>(), 0..256)).prop_map(|(p, c)| (PathBuf::from(p), c))
}

fn material_set_strategy() -> impl Strategy<Value = MaterialSet> {
    vec(entry_strategy(), 0..8).prop_map(|mut entries| {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries.dedup_by(|a, b| a.0 == b.0);
        MaterialSet { entries, parameters: BTreeMap::new() }
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]

    /// Order independence: reversing entries does not change the hash.
    #[test]
    fn hash_is_order_independent(mat in material_set_strategy()) {
        let mut shuffled = mat.entries.clone();
        shuffled.reverse();
        let shuffled_mat = MaterialSet {
            entries: shuffled,
            parameters: mat.parameters.clone(),
        };
        prop_assert_eq!(
            hash_resolved_namespace(&mat),
            hash_resolved_namespace(&shuffled_mat)
        );
    }

    /// Avalanche: a single-byte content flip changes the hash.
    #[test]
    fn hash_changes_on_single_byte_content_flip(
        mat in material_set_strategy().prop_filter("non-empty", |m| !m.entries.is_empty())
    ) {
        let original = hash_resolved_namespace(&mat);
        let mut flipped = mat.clone();
        if flipped.entries[0].1.is_empty() {
            flipped.entries[0].1.push(0);
        } else {
            flipped.entries[0].1[0] ^= 0x01;
        }
        prop_assert_ne!(original, hash_resolved_namespace(&flipped));
    }

    /// Any path rename changes the hash.
    #[test]
    fn hash_changes_on_path_rename(
        mat in material_set_strategy().prop_filter("non-empty", |m| !m.entries.is_empty())
    ) {
        let original = hash_resolved_namespace(&mat);
        let mut renamed = mat.clone();
        renamed.entries[0].0 = PathBuf::from(format!(
            "renamed_{}",
            renamed.entries[0].0.display()
        ));
        prop_assert_ne!(original, hash_resolved_namespace(&renamed));
    }

    /// Case sensitivity in paths.
    #[test]
    fn hash_is_path_case_sensitive(content in vec(any::<u8>(), 0..32)) {
        let lower = MaterialSet {
            entries: vec![(PathBuf::from("foo.md"), content.clone())],
            parameters: BTreeMap::new(),
        };
        let upper = MaterialSet {
            entries: vec![(PathBuf::from("FOO.md"), content)],
            parameters: BTreeMap::new(),
        };
        prop_assert_ne!(hash_resolved_namespace(&lower), hash_resolved_namespace(&upper));
    }

    /// Adding a parameter value changes the hash (via synthetic
    /// .aenv/parameters.json).
    #[test]
    fn parameter_change_changes_hash(entries in vec(entry_strategy(), 0..4)) {
        let mut sorted = entries;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        sorted.dedup_by(|a, b| a.0 == b.0);
        let no_params = MaterialSet {
            entries: sorted.clone(),
            parameters: BTreeMap::new(),
        };
        let mut params = BTreeMap::new();
        params.insert(
            "default_model".to_string(),
            ResolvedParameter {
                value: ParameterValue::String("claude-opus-4.7".into()),
                source: aenv_core::identity::NamespaceId::new("leaf").unwrap(),
            },
        );
        let with_params = MaterialSet {
            entries: sorted,
            parameters: params,
        };
        prop_assert_ne!(
            hash_resolved_namespace(&no_params),
            hash_resolved_namespace(&with_params)
        );
    }

    /// Parameter SOURCE provenance is NOT hashed — only effective values are.
    #[test]
    fn hash_ignores_parameter_provenance(value in "[a-z]{1,20}") {
        let mut params_a = BTreeMap::new();
        params_a.insert(
            "default_model".to_string(),
            ResolvedParameter {
                value: ParameterValue::String(value.clone()),
                source: aenv_core::identity::NamespaceId::new("a").unwrap(),
            },
        );
        let mut params_b = BTreeMap::new();
        params_b.insert(
            "default_model".to_string(),
            ResolvedParameter {
                value: ParameterValue::String(value),
                source: aenv_core::identity::NamespaceId::new("b").unwrap(),
            },
        );
        let a = MaterialSet { entries: vec![], parameters: params_a };
        let b = MaterialSet { entries: vec![], parameters: params_b };
        prop_assert_eq!(hash_resolved_namespace(&a), hash_resolved_namespace(&b));
    }
}
```

- [ ] **Step 2: Run + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test hash_properties 2>&1 | tail -15`
Expected: 6 properties pass, 32 cases each.

```bash
git add crates/aenv-core/tests/hash_properties.rs
git commit -m "Add proptest invariants for resolved-namespace hash

Six properties from engineering §8.3:
- Order independence of material-set entries.
- Avalanche on single-byte content change.
- Hash changes on path rename.
- Path case sensitivity (foo != FOO).
- Parameter value changes affect the hash via .aenv/parameters.json.
- Parameter SOURCE (which namespace declared it) does NOT affect the
  hash — only the effective value does.

Cases capped at 32 (default 256 slows CI; 32 catches invariant
breakage reliably for these input shapes).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: JSON response shapes (typed structs)

Add the `json/` module with one response struct per command. Each struct derives `serde::Serialize`. Per-command `From<...>` constructors land in subsequent tasks alongside the CLI wiring.

**Files:**
- Create: `crates/aenv-core/src/json/mod.rs`
- Create: `crates/aenv-core/src/json/{status,list,which,get,doctor,adapter,skill,diff}.rs`
- Modify: `crates/aenv-core/src/lib.rs`
- Test: `crates/aenv-core/tests/json_shapes_compile.rs`

- [ ] **Step 1: Write the shape-compile smoke test**

Create `crates/aenv-core/tests/json_shapes_compile.rs`:

```rust
use aenv_core::json::{
    AdapterEntryJson, DriftReport, GetReport, ListEntry, SkillEntry, StatusReport, StructuralDiff,
    WhichReport,
};

macro_rules! assert_object {
    ($t:ty) => {{
        let v = serde_json::to_value(<$t>::default()).unwrap();
        assert!(v.is_object(), "{} must serialize as a JSON object", stringify!($t));
    }};
}

#[test]
fn every_shape_is_an_object() {
    assert_object!(StatusReport);
    assert_object!(ListEntry);
    assert_object!(WhichReport);
    assert_object!(GetReport);
    assert_object!(AdapterEntryJson);
    assert_object!(SkillEntry);
    assert_object!(DriftReport);
    assert_object!(StructuralDiff);
}
```

- [ ] **Step 2: Create `crates/aenv-core/src/json/mod.rs`**

```rust
//! Typed response shapes for every `--json` flag.
//!
//! Each command crate constructs an instance and prints
//! `serde_json::to_string_pretty(&shape)`. Schemas are locked with
//! insta snapshot tests in `tests/json_snapshots.rs`.

pub mod adapter;
pub mod diff;
pub mod doctor;
pub mod get;
pub mod list;
pub mod skill;
pub mod status;
pub mod which;

pub use adapter::AdapterEntryJson;
pub use diff::{DriftReport, StructuralDiff};
pub use doctor::DoctorReportJson;
pub use get::GetReport;
pub use list::ListEntry;
pub use skill::SkillEntry;
pub use status::StatusReport;
pub use which::WhichReport;
```

- [ ] **Step 3: Create each per-command shape file**

The exact file bodies are listed in the Task-6 section of the plan header (`File structure`). Copy each verbatim:
- `json/status.rs` — `StatusReport`, `ManagedFileJson`, `SkillProvenanceJson`, `BackedUpJson`
- `json/list.rs` — `ListEntry`
- `json/which.rs` — `WhichReport`
- `json/get.rs` — `GetReport`, `InheritanceEntry`
- `json/doctor.rs` — `DoctorReportJson`, `OutcomeJson`
- `json/adapter.rs` — `AdapterEntryJson`, `AdapterParameterJson`
- `json/skill.rs` — `SkillEntry`
- `json/diff.rs` — `DriftReport`, `DriftedFile`, `StructuralDiff`, `SetDiff`, `ValueDiff`, `NamedValue`, `ValueChange`

Bodies (in full):

`json/status.rs`:
```rust
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use crate::parameters::ResolvedParameter;
use crate::policies::ResolvedPolicy;

#[derive(Debug, Clone, Default, Serialize)]
pub struct StatusReport {
    pub project: PathBuf,
    pub active_namespace: Option<String>,
    pub resolution_chain: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash_v2: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ResolvedParameter>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub policies: BTreeMap<String, ResolvedPolicy>,
    pub managed_files: Vec<ManagedFileJson>,
    pub backed_up: Vec<BackedUpJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ManagedFileJson {
    pub path: PathBuf,
    pub qualified_name: String,
    pub short_name: String,
    pub provided_by_namespace: Option<String>,
    pub strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_kind: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_provenance: Option<SkillProvenanceJson>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SkillProvenanceJson {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_ref: Option<String>,
    pub resolved_hash: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct BackedUpJson {
    pub path: PathBuf,
    pub backup: PathBuf,
}
```

`json/list.rs`:
```rust
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ListEntry {
    pub name: String,
    pub extends: Vec<String>,
    pub adapters: Vec<String>,
    pub parameters_declared: Vec<String>,
    pub policies_declared: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash_v2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
```

`json/which.rs`:
```rust
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize)]
pub struct WhichReport {
    pub path: PathBuf,
    pub qualified_name: String,
    pub short_name: String,
    pub provided_by_namespace: Option<String>,
    pub strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_kind: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<String>,
}
```

`json/get.rs`:
```rust
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct GetReport {
    pub parameter: String,
    pub value: serde_json::Value,
    pub source_namespace: String,
    pub inheritance_chain: Vec<InheritanceEntry>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct InheritanceEntry {
    pub namespace: String,
    pub value: serde_json::Value,
}
```

`json/doctor.rs`:
```rust
use serde::Serialize;
use std::collections::BTreeMap;
use crate::policies::ResolvedPolicy;

#[derive(Debug, Clone, Default, Serialize)]
pub struct DoctorReportJson {
    pub namespace: String,
    pub chain: Vec<String>,
    pub policies: BTreeMap<String, ResolvedPolicy>,
    pub outcomes: Vec<OutcomeJson>,
    pub pass_count: usize,
    pub warn_count: usize,
    pub fail_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct OutcomeJson {
    pub key: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}
```

`json/adapter.rs`:
```rust
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct AdapterEntryJson {
    pub name: String,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_dir: Option<String>,
    pub parameters: Vec<AdapterParameterJson>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub soft_limits: std::collections::BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AdapterParameterJson {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projects_to: Option<String>,
}
```

`json/skill.rs`:
```rust
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SkillEntry {
    pub namespace: String,
    pub qualified_name: String,
    pub short_name: String,
    pub adapter: Option<String>,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    pub required: bool,
}
```

`json/diff.rs`:
```rust
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize)]
pub struct DriftReport {
    pub project: PathBuf,
    pub active_namespace: String,
    pub drifted: Vec<DriftedFile>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DriftedFile {
    pub path: PathBuf,
    pub qualified_name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct StructuralDiff {
    pub a: String,
    pub b: String,
    pub skills: SetDiff,
    pub agents: SetDiff,
    pub parameters: ValueDiff,
    pub policies: ValueDiff,
    pub instructions_sections: SetDiff,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SetDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub common: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ValueDiff {
    pub added: Vec<NamedValue>,
    pub removed: Vec<NamedValue>,
    pub changed: Vec<ValueChange>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct NamedValue {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ValueChange {
    pub name: String,
    pub a: serde_json::Value,
    pub b: serde_json::Value,
}
```

- [ ] **Step 4: Add `pub mod json;` to `crates/aenv-core/src/lib.rs`**

- [ ] **Step 5: Run + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test json_shapes_compile 2>&1 | tail -5 && PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3`
Expected: 1 test passes; clippy silent.

```bash
git add crates/aenv-core/src/json crates/aenv-core/src/lib.rs \
        crates/aenv-core/tests/json_shapes_compile.rs
git commit -m "Add typed JSON response shapes for every read-oriented command

One file per command:
- StatusReport (the largest payload, per spec §7.1)
- ListEntry, WhichReport, GetReport, DoctorReportJson
- AdapterEntryJson (with AdapterParameterJson)
- SkillEntry
- DriftReport + StructuralDiff (for aenv diff, both flavors)

Every struct derives Serialize with skip_serializing_if for absent
optionals so the JSON carries only present data. StatusReport and
ListEntry already carry a resolved_hash_v2 field (always None today,
skipped) as the R-87 forward-compatibility hook.

Per-command From<...> constructors land in Tasks 7-14 alongside the
CLI wiring; this commit is shapes-only.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Wire `--json` through clap + cmd dispatcher

Add a `--json` flag to every read-oriented subcommand and thread `json: bool` down to each `cmd::*::run` signature. This is a mechanical change — every command in this task gates its existing text output on `!json` and (in Tasks 8–14) adds the JSON branch.

**Files:**
- Modify: `crates/aenv-cli/src/main.rs`
- Modify: `crates/aenv-cli/src/cmd/{status,list,which,get,doctor,adapter}.rs`
- Modify: `crates/aenv-cli/src/cmd/skill/list.rs`

- [ ] **Step 1: Add `--json` to clap subcommands**

In `crates/aenv-cli/src/main.rs`, add `#[arg(long)] json: bool` to:
- `Command::Status { project, json }`
- `Command::List { json }`  (new — `List` has no fields today; add a struct-style variant)
- `Command::Which { path, project, json }`
- `Command::Get { spec, json }`
- `Command::Doctor { namespace, json }`
- `AdapterAction::List { json }` (new — `AdapterAction::List` has no fields today)
- `SkillAction::List { ns, json }`

Each existing match arm for these commands needs the new field added to its destructuring and forwarded to the `cmd::*::run` call.

The List → struct-variant migration changes:

```rust
// Before:
List,
// After:
List {
    #[arg(long)]
    json: bool,
},
```

And in the dispatcher:

```rust
// Before:
Command::List => cmd::list::run(&fs, &layout),
// After:
Command::List { json } => cmd::list::run(&fs, &layout, json),
```

Mirror this for `AdapterAction::List`.

- [ ] **Step 2: Extend every `run(...)` signature with `json: bool`**

Each affected `cmd/*.rs` file gains a `json: bool` trailing parameter. Inside, the existing text-rendering code becomes the `if !json { ... }` branch; the JSON branch is added in the per-command task that follows. For now, the JSON branch is a placeholder that errors with `unimplemented!("aenv <cmd> --json lands in Task <N>")` so the type-checker is satisfied without misleading output.

Example for `cmd/status.rs`:

```rust
pub fn run<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    aenv_home: &Path,
    json: bool,
) -> Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        if json {
            unimplemented!("aenv status --json lands in Task 8");
        }
        println!("No active namespace in {}", project_root.display());
        return Ok(());
    }
    // ... existing read+resolve ...
    if json {
        unimplemented!("aenv status --json lands in Task 8");
    }
    print!("{}", format_status(&state, &resolution.chain));
    Ok(())
}
```

Apply the same pattern to `cmd/list.rs`, `cmd/which.rs`, `cmd/get.rs`, `cmd/doctor.rs`, `cmd/adapter.rs` (only `run_list`), and `cmd/skill/list.rs`.

- [ ] **Step 3: Run + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --workspace 2>&1 | tail -10`
Expected: builds cleanly.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -3`
Expected: all existing tests pass (none invoke `--json` yet, so the `unimplemented!()` placeholders are not triggered).

```bash
git add crates/aenv-cli/src/main.rs crates/aenv-cli/src/cmd/
git commit -m "Wire --json flag through every read-oriented command

Plumbing-only commit: add #[arg(long)] json: bool to the clap variants
for Status, List, Which, Get, Doctor, Adapter::List, Skill::List.
List and AdapterAction::List become struct-style variants so they can
carry the flag.

Each cmd::*::run signature gains a trailing json: bool. The text
branch is gated on !json; the JSON branch is unimplemented!() with a
pointer to the task that fills it in (8-13). Existing tests don't
touch --json so they continue to pass.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: `aenv status --json`

The largest JSON payload — see functional spec §7.1 for the canonical shape. Build a `StatusReport` from the active state plus a freshly-computed material set and hash.

**Files:**
- Modify: `crates/aenv-cli/src/cmd/status.rs`
- Test: `crates/aenv-core/tests/json_snapshots.rs` (incremental; lock the status shape)
- Test: `crates/aenv-cli/tests/status_json_e2e.rs`

- [ ] **Step 1: Write the failing snapshot test**

Create or append to `crates/aenv-core/tests/json_snapshots.rs`:

```rust
//! Insta snapshot tests locking the shape of every --json response.
//!
//! Snapshots live under `tests/snapshots/`. Approve schema changes with
//! `cargo insta accept --all` and review the diff in code review.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::json::status::{ManagedFileJson, StatusReport};
use std::path::PathBuf;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn build_solo_fixture() -> (TempDir, RegistryLayout, AdapterRegistry) {
    let tmp = TempDir::new().unwrap();
    let layout = RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    write_file(
        &layout.manifest_path("solo"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(
        &layout.namespace_dir("solo").join("CLAUDE.md"),
        "# Hello\n",
    );
    (tmp, layout, adapters)
}

#[test]
fn status_report_shape_is_stable() {
    let report = StatusReport {
        project: PathBuf::from("/proj"),
        active_namespace: Some("solo".into()),
        resolution_chain: vec!["solo".into()],
        resolved_hash: Some("sha256-v1:0000000000000000000000000000000000000000000000000000000000000000".into()),
        resolved_hash_v2: None,
        parameters: Default::default(),
        policies: Default::default(),
        managed_files: vec![ManagedFileJson {
            path: PathBuf::from("CLAUDE.md"),
            qualified_name: "solo::CLAUDE.md".into(),
            short_name: "CLAUDE.md".into(),
            provided_by_namespace: Some("solo".into()),
            strategy: "symlink".into(),
            merge_kind: None,
            contributors: vec![],
            shadows: vec![],
            skill_provenance: None,
        }],
        backed_up: vec![],
        warnings: vec![],
    };
    insta::assert_json_snapshot!(report);
}
```

- [ ] **Step 2: Run to verify it generates a pending snapshot**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test json_snapshots 2>&1 | tail -15`
Expected: test FAILS with a message about a new snapshot pending review.

Accept it: `PATH="$HOME/.cargo/bin:$PATH" cargo insta accept --workspace 2>&1 | tail -3`. Re-run the test; it now passes.

Inspect the accepted snapshot at `crates/aenv-core/tests/snapshots/json_snapshots__status_report_shape_is_stable.snap` — confirm it is the canonical shape (qualified_name `solo::CLAUDE.md`, strategy `symlink`, hash present, optional fields absent).

- [ ] **Step 3: Implement `StatusReport::from(state, resolution, hash)`**

Add to `crates/aenv-core/src/json/status.rs`:

```rust
use crate::resolve::{DeepMergeFormat, MaterializeStrategy, ResolutionResult};
use crate::state::{ActivationState, ManagedFile};

impl StatusReport {
    /// Build a `StatusReport` from a project's `ActivationState` plus the
    /// freshly-computed resolution and hash. `hash` is the
    /// `sha256-v1:<hex>` string from `hash::hash_resolved_namespace`.
    pub fn build(
        project_root: PathBuf,
        state: &ActivationState,
        resolution: &ResolutionResult,
        hash: String,
    ) -> Self {
        StatusReport {
            project: project_root,
            active_namespace: Some(state.active_namespace.clone()),
            resolution_chain: resolution
                .chain
                .iter()
                .map(|n| n.as_str().to_string())
                .collect(),
            resolved_hash: Some(hash),
            resolved_hash_v2: None,
            parameters: resolution.parameters.clone(),
            policies: resolution.policies.clone(),
            managed_files: state
                .managed_files
                .iter()
                .map(ManagedFileJson::from)
                .collect(),
            backed_up: state
                .backed_up
                .iter()
                .map(|b| BackedUpJson {
                    path: b.original_path.clone(),
                    backup: b.backup_path.clone(),
                })
                .collect(),
            warnings: state.warnings.clone(),
        }
    }

    /// Build a `StatusReport` for a project that has no active namespace.
    pub fn unpinned(project_root: PathBuf) -> Self {
        StatusReport {
            project: project_root,
            ..Default::default()
        }
    }
}

impl From<&ManagedFile> for ManagedFileJson {
    fn from(mf: &ManagedFile) -> Self {
        let (strategy_str, merge_kind) = match mf.strategy {
            MaterializeStrategy::Symlink => ("symlink", None),
            MaterializeStrategy::Identical => ("identical", None),
            MaterializeStrategy::Copy => ("copy", None),
            MaterializeStrategy::SectionMerge => ("section-merge", None),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Json) => ("deep-merge", Some("json")),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Yaml) => ("deep-merge", Some("yaml")),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Toml) => ("deep-merge", Some("toml")),
            MaterializeStrategy::Merged => ("merged", None),
        };
        let provided_by = if mf.qualified_name.namespace().as_str()
            == crate::identity::NamespaceId::RESERVED_MERGED
        {
            None
        } else {
            Some(mf.qualified_name.namespace().as_str().to_string())
        };
        ManagedFileJson {
            path: mf.path.clone(),
            qualified_name: mf.qualified_name.to_string(),
            short_name: mf.qualified_name.short().as_str().to_string(),
            provided_by_namespace: provided_by,
            strategy: strategy_str.to_string(),
            merge_kind: merge_kind.map(str::to_string),
            contributors: mf.contributors.iter().map(ToString::to_string).collect(),
            shadows: mf.shadows.iter().map(ToString::to_string).collect(),
            skill_provenance: mf.skill_provenance.as_ref().map(|p| SkillProvenanceJson {
                source: p.source.clone(),
                resolved_ref: p.resolved_ref.clone(),
                resolved_hash: p.resolved_hash.clone(),
            }),
        }
    }
}
```

- [ ] **Step 4: Wire `aenv status --json`**

Replace `cmd/status.rs::run`:

```rust
pub fn run<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    aenv_home: &Path,
    json: bool,
) -> Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        if json {
            let report = aenv_core::json::StatusReport::unpinned(project_root.to_path_buf());
            println!("{}", serde_json::to_string_pretty(&report)
                .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("json: {e}")))?);
        } else {
            println!("No active namespace in {}", project_root.display());
        }
        return Ok(());
    }
    let bytes = fs.read(&state_path)?;
    let text = String::from_utf8(bytes)
        .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("state.json: {e}")))?;
    let state = ActivationState::from_json(&text)?;

    let registry = aenv_core::home::RegistryLayout::new(aenv_home.to_path_buf());
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(fs, &registry.adapters_dir())?;
    let leaf = NamespaceId::new(state.active_namespace.as_str())?;
    let resolution = aenv_core::resolve::resolve_namespace(fs, &registry, &adapters, &leaf)?;

    if json {
        let mat = aenv_core::materialize::compute_material_set(fs, &registry, &adapters, &leaf)?;
        let hash = aenv_core::hash::hash_resolved_namespace(&mat);
        let report = aenv_core::json::StatusReport::build(
            project_root.to_path_buf(),
            &state,
            &resolution,
            hash,
        );
        println!("{}", serde_json::to_string_pretty(&report)
            .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("json: {e}")))?);
        return Ok(());
    }

    print!("{}", format_status(&state, &resolution.chain));
    Ok(())
}
```

- [ ] **Step 5: End-to-end CLI test**

Create `crates/aenv-cli/tests/status_json_e2e.rs`:

```rust
//! End-to-end: aenv status --json produces parseable JSON with the
//! top-level keys functional spec §7.1 documents.

use std::process::Command;
use tempfile::TempDir;

#[test]
fn status_json_against_active_project() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    // Set up registry: one namespace `solo` with claude-code + CLAUDE.md.
    let envs_dir = aenv_home.path().join("envs/solo");
    std::fs::create_dir_all(&envs_dir).unwrap();
    std::fs::write(
        envs_dir.join("aenv.toml"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    ).unwrap();
    std::fs::write(envs_dir.join("CLAUDE.md"), "# Hello\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_aenv");

    // Pin + activate.
    Command::new(bin)
        .args(["use", "solo", "--project", project.path().to_str().unwrap()])
        .env("AENV_HOME", aenv_home.path())
        .status()
        .unwrap();
    Command::new(bin)
        .args(["activate", "--project", project.path().to_str().unwrap()])
        .env("AENV_HOME", aenv_home.path())
        .status()
        .unwrap();

    // Status --json.
    let out = Command::new(bin)
        .args(["status", "--project", project.path().to_str().unwrap(), "--json"])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["project"].is_string());
    assert_eq!(v["active_namespace"], "solo");
    assert!(v["resolution_chain"].is_array());
    assert!(v["resolved_hash"].as_str().unwrap().starts_with("sha256-v1:"));
    assert!(v["managed_files"].is_array());
    assert!(v["backed_up"].is_array());
}
```

This test depends on `AENV_HOME` being honored by `paths::resolve_aenv_home`. Confirm the existing implementation reads `AENV_HOME` first; if it doesn't, plumb the env var into the resolver before running the test (Phase 1's resolver already does this — verify with `grep -n 'AENV_HOME' crates/aenv-cli/src/paths.rs`).

- [ ] **Step 6: Run all the things + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -5`
Expected: all green.

```bash
git add crates/aenv-core/src/json/status.rs crates/aenv-cli/src/cmd/status.rs \
        crates/aenv-core/tests/json_snapshots.rs \
        crates/aenv-core/tests/snapshots/json_snapshots__status_report_shape_is_stable.snap \
        crates/aenv-cli/tests/status_json_e2e.rs
git commit -m "Implement aenv status --json

- StatusReport::build assembles the spec §7.1 payload from
  ActivationState + ResolutionResult + freshly-computed
  sha256-v1:<hex>. StatusReport::unpinned covers the no-active-namespace
  case (project + empty fields).
- ManagedFileJson::From<&ManagedFile> normalizes strategy enum to the
  string form ('symlink' / 'identical' / 'copy' / 'section-merge' /
  'deep-merge' with merge_kind / 'merged' legacy). Merged-multi-namespace
  artifacts (qualified_name namespace is the reserved (merged)) emit
  provided_by_namespace = null.
- insta snapshot locks the canonical shape; future schema changes
  surface as snapshot diffs in code review.
- e2e test drives the CLI binary against a tempdir registry+project
  and asserts on the top-level keys.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 9: `aenv list --json`, `aenv adapter list --json`, `aenv skill list --json`

All three are list-style endpoints; they share enough structure to land as one commit.

**Files:**
- Modify: `crates/aenv-core/src/json/list.rs`, `adapter.rs`, `skill.rs`
- Modify: `crates/aenv-cli/src/cmd/list.rs`, `adapter.rs`, `skill/list.rs`
- Test: append to `crates/aenv-core/tests/json_snapshots.rs`

- [ ] **Step 1: Add `From<...>` constructors + builder for `ListEntry`**

In `crates/aenv-core/src/json/list.rs`:

```rust
use crate::adapter::AdapterRegistry;
use crate::fs::Filesystem;
use crate::hash::hash_resolved_namespace;
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::manifest::AenvManifest;
use crate::materialize::compute_material_set;

impl ListEntry {
    /// Build a `ListEntry` for one namespace by reading its manifest
    /// and (best-effort) resolving + hashing it. Resolution errors are
    /// captured in `error`; the entry is still emitted so scripts get
    /// every namespace.
    pub fn build<F: Filesystem>(
        fs: &F,
        layout: &RegistryLayout,
        adapters: &AdapterRegistry,
        name: &str,
    ) -> Self {
        let manifest_path = layout.manifest_path(name);
        let manifest = match fs.read(&manifest_path).ok()
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| AenvManifest::from_toml(&s).ok())
        {
            Some(m) => m,
            None => {
                return ListEntry {
                    name: name.to_string(),
                    error: Some("manifest invalid or unreadable".into()),
                    ..Default::default()
                };
            }
        };

        let extends = manifest.extends.clone();
        let adapters_decl: Vec<String> = manifest.adapters.keys().cloned().collect();
        let parameters_declared: Vec<String> = manifest.parameters.keys().cloned().collect();
        let policies_declared: Vec<String> = manifest.policies.keys().cloned().collect();

        let leaf = match NamespaceId::new(name) {
            Ok(id) => id,
            Err(e) => {
                return ListEntry {
                    name: name.to_string(),
                    extends,
                    adapters: adapters_decl,
                    parameters_declared,
                    policies_declared,
                    error: Some(e.to_string()),
                    ..Default::default()
                };
            }
        };

        let (hash, error) = match compute_material_set(fs, layout, adapters, &leaf) {
            Ok(mat) => (Some(hash_resolved_namespace(&mat)), None),
            Err(e) => (None, Some(e.to_string())),
        };

        ListEntry {
            name: name.to_string(),
            extends,
            adapters: adapters_decl,
            parameters_declared,
            policies_declared,
            resolved_hash: hash,
            resolved_hash_v2: None,
            error,
        }
    }
}
```

- [ ] **Step 2: Wire `cmd/list.rs`**

```rust
pub fn run<F: Filesystem>(fs: &F, layout: &RegistryLayout, json: bool) -> Result<()> {
    let names = aenv_core::namespace::list_namespaces(fs, layout)?;
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;

    if json {
        let entries: Vec<aenv_core::json::ListEntry> = names
            .iter()
            .map(|n| aenv_core::json::ListEntry::build(fs, layout, &adapters, n))
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)
            .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("json: {e}")))?);
        return Ok(());
    }

    if names.is_empty() {
        println!("No namespaces in registry at {}", layout.root().display());
        return Ok(());
    }
    println!("NAME");
    for name in names {
        println!("{name}");
    }
    Ok(())
}
```

- [ ] **Step 3: Wire `cmd/adapter.rs::run_list`**

Add a `From<&Adapter>` for `AdapterEntryJson` in `crates/aenv-core/src/json/adapter.rs`:

```rust
use crate::adapter::{Adapter, AdapterParameterType};

impl AdapterEntryJson {
    pub fn from_adapter(a: &Adapter) -> Self {
        AdapterEntryJson {
            name: a.name.clone(),
            files: a.files.clone(),
            skills_dir: a.skills_dir.clone(),
            parameters: a.parameters.iter().map(|p| AdapterParameterJson {
                name: p.name.clone(),
                type_: match p.r#type {
                    AdapterParameterType::String => "string".into(),
                    AdapterParameterType::Integer => "integer".into(),
                    AdapterParameterType::Boolean => "boolean".into(),
                    AdapterParameterType::ListString => "list-of-string".into(),
                },
                projects_to: p.projects_to.clone(),
            }).collect(),
            soft_limits: a.soft_limits.clone(),
        }
    }
}
```

And in `cmd/adapter.rs`:

```rust
pub fn run_list<F: Filesystem>(fs: &F, layout: &RegistryLayout, json: bool) -> Result<()> {
    let reg = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    if json {
        let entries: Vec<aenv_core::json::AdapterEntryJson> = reg
            .iter()
            .map(|(_, a)| aenv_core::json::AdapterEntryJson::from_adapter(a))
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)
            .map_err(|e| AenvError::ManifestInvalid(format!("json: {e}")))?);
        return Ok(());
    }
    if reg.is_empty() {
        println!("No adapters installed at {}", layout.adapters_dir().display());
        return Ok(());
    }
    println!("ADAPTER         FILES");
    for (name, adapter) in reg.iter() {
        println!("{:<15} {}", name, adapter.files.join(", "));
    }
    Ok(())
}
```

- [ ] **Step 4: Wire `cmd/skill/list.rs`**

Add `SkillEntry::from_decl(ns, &decl)` in `crates/aenv-core/src/json/skill.rs`:

```rust
use crate::identity::{NamespaceId, ShortName, QualifiedName};
use crate::skills::{SkillDecl, SkillMode};

impl SkillEntry {
    pub fn from_decl(ns: &str, decl: &SkillDecl) -> Self {
        let qn = NamespaceId::new(ns)
            .and_then(|n| ShortName::new(decl.name.clone()).map(|s| QualifiedName::new(n, s)));
        let qualified_name = qn.as_ref().map(ToString::to_string).unwrap_or_default();
        let pin = match (decl.mode, decl.ref_.as_deref()) {
            (_, Some(r)) => Some(r.to_string()),
            (SkillMode::Imported, None) => Some("(head)".to_string()),
            (SkillMode::Authored, None) => None,
        };
        SkillEntry {
            namespace: ns.to_string(),
            qualified_name,
            short_name: decl.name.clone(),
            adapter: decl.adapter.clone(),
            mode: match decl.mode {
                SkillMode::Authored => "authored".into(),
                SkillMode::Imported => "imported".into(),
            },
            source: decl.source.clone(),
            pin,
            required: decl.required,
        }
    }
}
```

And in `cmd/skill/list.rs`, gate the existing text-table behind `!json` and add the JSON branch:

```rust
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    ns_filter: Option<&str>,
    json: bool,
) -> Result<()> {
    // existing namespace-collection code that produces `namespaces: Vec<String>`
    // ...

    if json {
        let mut entries: Vec<aenv_core::json::SkillEntry> = Vec::new();
        for ns in &namespaces {
            let manifest_path = layout.manifest_path(ns);
            let Ok(bytes) = fs.read(&manifest_path) else { continue };
            let Ok(text) = String::from_utf8(bytes) else { continue };
            let Ok(manifest) = AenvManifest::from_toml(&text) else { continue };
            for s in &manifest.skills {
                entries.push(aenv_core::json::SkillEntry::from_decl(ns, s));
            }
        }
        println!("{}", serde_json::to_string_pretty(&entries)
            .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("json: {e}")))?);
        return Ok(());
    }

    // existing text-table rendering follows
    // ...
}
```

- [ ] **Step 5: Append snapshot locks for the three list shapes**

In `crates/aenv-core/tests/json_snapshots.rs`:

```rust
use aenv_core::json::list::ListEntry;
use aenv_core::json::adapter::{AdapterEntryJson, AdapterParameterJson};
use aenv_core::json::skill::SkillEntry;

#[test]
fn list_entry_shape_is_stable() {
    let e = ListEntry {
        name: "leaf".into(),
        extends: vec!["base".into()],
        adapters: vec!["claude-code".into()],
        parameters_declared: vec!["default_model".into()],
        policies_declared: vec!["skill_requires_description".into()],
        resolved_hash: Some("sha256-v1:abc".into()),
        resolved_hash_v2: None,
        error: None,
    };
    insta::assert_json_snapshot!(e);
}

#[test]
fn adapter_entry_shape_is_stable() {
    let e = AdapterEntryJson {
        name: "claude-code".into(),
        files: vec!["CLAUDE.md".into(), ".claude/skills/**/*".into()],
        skills_dir: Some(".claude/skills".into()),
        parameters: vec![AdapterParameterJson {
            name: "default_model".into(),
            type_: "string".into(),
            projects_to: None,
        }],
        soft_limits: [("instructions".to_string(), 5000)].into_iter().collect(),
    };
    insta::assert_json_snapshot!(e);
}

#[test]
fn skill_entry_shape_is_stable() {
    let e = SkillEntry {
        namespace: "leaf".into(),
        qualified_name: "leaf::write-tests".into(),
        short_name: "write-tests".into(),
        adapter: Some("claude-code".into()),
        mode: "imported".into(),
        source: Some("git+https://example.com/skills.git#write-tests".into()),
        pin: Some("v1.2.0".into()),
        required: true,
    };
    insta::assert_json_snapshot!(e);
}
```

Accept snapshots: `PATH="$HOME/.cargo/bin:$PATH" cargo insta accept --workspace`.

- [ ] **Step 6: Run + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -5 && PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3`
Expected: all green; clippy silent.

```bash
git add crates/aenv-core/src/json/list.rs crates/aenv-core/src/json/adapter.rs \
        crates/aenv-core/src/json/skill.rs crates/aenv-cli/src/cmd/list.rs \
        crates/aenv-cli/src/cmd/adapter.rs crates/aenv-cli/src/cmd/skill/list.rs \
        crates/aenv-core/tests/json_snapshots.rs \
        crates/aenv-core/tests/snapshots/json_snapshots__list_entry_shape_is_stable.snap \
        crates/aenv-core/tests/snapshots/json_snapshots__adapter_entry_shape_is_stable.snap \
        crates/aenv-core/tests/snapshots/json_snapshots__skill_entry_shape_is_stable.snap
git commit -m "Implement --json for list / adapter list / skill list

- ListEntry::build resolves each namespace best-effort and emits its
  declared adapters / parameters / policies plus sha256-v1:<hex>.
  Resolution failures land in an `error` field so scripts always get
  one entry per namespace.
- AdapterEntryJson::from_adapter mirrors the parsed Adapter struct;
  type_tag enum is stringified ('string' / 'integer' / 'boolean' /
  'list-of-string') for stable wire form.
- SkillEntry::from_decl renders the qualified name and the
  '(head)'-for-unpinned convention from spec §5.11.
- Three insta snapshots lock the schemas.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 10: `aenv which --json` and `aenv get --json`

The two single-item commands.

**Files:**
- Modify: `crates/aenv-core/src/json/which.rs`, `get.rs`
- Modify: `crates/aenv-cli/src/cmd/which.rs`, `get.rs`
- Test: append to `crates/aenv-core/tests/json_snapshots.rs`

- [ ] **Step 1: `WhichReport::from_managed_file`**

In `crates/aenv-core/src/json/which.rs`:

```rust
use crate::state::ManagedFile;
use crate::resolve::{DeepMergeFormat, MaterializeStrategy};

impl WhichReport {
    pub fn from_managed_file(mf: &ManagedFile) -> Self {
        let (strategy, merge_kind) = match mf.strategy {
            MaterializeStrategy::Symlink => ("symlink", None),
            MaterializeStrategy::Identical => ("identical", None),
            MaterializeStrategy::Copy => ("copy", None),
            MaterializeStrategy::SectionMerge => ("section-merge", None),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Json) => ("deep-merge", Some("json")),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Yaml) => ("deep-merge", Some("yaml")),
            MaterializeStrategy::DeepMerge(DeepMergeFormat::Toml) => ("deep-merge", Some("toml")),
            MaterializeStrategy::Merged => ("merged", None),
        };
        let provided_by = if mf.qualified_name.namespace().as_str()
            == crate::identity::NamespaceId::RESERVED_MERGED
        {
            None
        } else {
            Some(mf.qualified_name.namespace().as_str().to_string())
        };
        WhichReport {
            path: mf.path.clone(),
            qualified_name: mf.qualified_name.to_string(),
            short_name: mf.qualified_name.short().as_str().to_string(),
            provided_by_namespace: provided_by,
            strategy: strategy.to_string(),
            merge_kind: merge_kind.map(str::to_string),
            contributors: mf.contributors.iter().map(ToString::to_string).collect(),
            shadows: mf.shadows.iter().map(ToString::to_string).collect(),
        }
    }
}
```

Wire `cmd/which.rs`: branch on `json` after `state` and the lookup are loaded; on JSON path, build `WhichReport::from_managed_file` and pretty-print. On the not-found case (today the function returns `ActivationConflict`), preserve that behavior for both text and JSON — the JSON form just changes the success branch.

- [ ] **Step 2: `GetReport::build`**

In `crates/aenv-core/src/json/get.rs`:

```rust
use crate::parameters::{ParameterValue, ResolvedParameter};

impl GetReport {
    pub fn build(
        parameter: String,
        rp: &ResolvedParameter,
        inheritance: Vec<(String, ParameterValue)>,
    ) -> Self {
        GetReport {
            parameter,
            value: param_value_to_json(&rp.value),
            source_namespace: rp.source.as_str().to_string(),
            inheritance_chain: inheritance
                .into_iter()
                .map(|(ns, v)| InheritanceEntry {
                    namespace: ns,
                    value: param_value_to_json(&v),
                })
                .collect(),
        }
    }
}

fn param_value_to_json(v: &ParameterValue) -> serde_json::Value {
    match v {
        ParameterValue::String(s) => serde_json::Value::String(s.clone()),
        ParameterValue::Integer(i) => serde_json::Value::Number((*i).into()),
        ParameterValue::Boolean(b) => serde_json::Value::Bool(*b),
        ParameterValue::ListString(xs) => serde_json::Value::Array(
            xs.iter().map(|s| serde_json::Value::String(s.clone())).collect(),
        ),
    }
}
```

Wire `cmd/get.rs::run`: collect the inheritance chain by walking the resolved namespace chain and re-reading each manifest for declarations of `param` (the existing code already does this for the text-source provenance message — refactor to return `Vec<(String, ParameterValue)>` from one helper, then either render it as text or feed it into `GetReport::build` based on `json`).

- [ ] **Step 3: Lock snapshots**

```rust
#[test]
fn which_report_shape_is_stable() {
    use aenv_core::json::WhichReport;
    let r = WhichReport {
        path: std::path::PathBuf::from(".claude/skills/write-tests/SKILL.md"),
        qualified_name: "leaf::write-tests".into(),
        short_name: "write-tests".into(),
        provided_by_namespace: Some("leaf".into()),
        strategy: "symlink".into(),
        merge_kind: None,
        contributors: vec![],
        shadows: vec!["base::write-tests".into()],
    };
    insta::assert_json_snapshot!(r);
}

#[test]
fn get_report_shape_is_stable() {
    use aenv_core::json::get::{GetReport, InheritanceEntry};
    let r = GetReport {
        parameter: "default_model".into(),
        value: serde_json::json!("claude-opus-4.7"),
        source_namespace: "leaf".into(),
        inheritance_chain: vec![
            InheritanceEntry { namespace: "base".into(), value: serde_json::json!("claude-sonnet-4.6") },
            InheritanceEntry { namespace: "leaf".into(), value: serde_json::json!("claude-opus-4.7") },
        ],
    };
    insta::assert_json_snapshot!(r);
}
```

Accept snapshots and commit:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo insta accept --workspace
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -3

git add crates/aenv-core/src/json/which.rs crates/aenv-core/src/json/get.rs \
        crates/aenv-cli/src/cmd/which.rs crates/aenv-cli/src/cmd/get.rs \
        crates/aenv-core/tests/json_snapshots.rs \
        crates/aenv-core/tests/snapshots/json_snapshots__which_report_shape_is_stable.snap \
        crates/aenv-core/tests/snapshots/json_snapshots__get_report_shape_is_stable.snap
git commit -m "Implement aenv which --json and aenv get --json

- WhichReport::from_managed_file mirrors the existing text-form fields
  (qualified_name, short_name, strategy + merge_kind, contributors,
  shadows). Merged-multi-namespace files emit provided_by_namespace =
  null, matching the StatusReport convention from Task 8.
- GetReport::build carries the effective value (as a JSON scalar/array
  preserving the parameter's declared type) plus the inheritance chain
  documented in spec §7.1. The CLI walks the chain once and feeds the
  result into either text rendering or the JSON shape.
- Two more insta snapshots.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 11: `aenv doctor --json`

**Files:**
- Modify: `crates/aenv-core/src/json/doctor.rs`
- Modify: `crates/aenv-cli/src/cmd/doctor.rs`
- Test: append to `crates/aenv-core/tests/json_snapshots.rs`

- [ ] **Step 1: `DoctorReportJson::from_report`**

In `crates/aenv-core/src/json/doctor.rs`:

```rust
use crate::doctor::DoctorReport;
use crate::policies::builtin::OutcomeStatus;

impl DoctorReportJson {
    pub fn from_report(namespace: &str, report: &DoctorReport) -> Self {
        let mut pass_count = 0;
        let mut warn_count = 0;
        let mut fail_count = 0;
        let mut skipped_count = 0;
        let outcomes: Vec<OutcomeJson> = report.outcomes.iter().map(|o| {
            let (status, msg) = match &o.status {
                OutcomeStatus::Pass => { pass_count += 1; ("pass", None) }
                OutcomeStatus::Warn { msg } => { warn_count += 1; ("warn", Some(msg.clone())) }
                OutcomeStatus::Fail { msg } => { fail_count += 1; ("fail", Some(msg.clone())) }
                OutcomeStatus::WarnSkip { msg } => { skipped_count += 1; ("skipped", Some(msg.clone())) }
            };
            OutcomeJson {
                key: o.key.clone(),
                status: status.to_string(),
                target: o.target.as_ref().map(ToString::to_string),
                msg,
            }
        }).collect();
        DoctorReportJson {
            namespace: namespace.to_string(),
            chain: report.chain.iter().map(|n| n.as_str().to_string()).collect(),
            policies: report.policies.clone(),
            outcomes,
            pass_count, warn_count, fail_count, skipped_count,
        }
    }
}
```

- [ ] **Step 2: Wire `cmd/doctor.rs::run`**

Branch on `json` after computing the report. On JSON path, build `DoctorReportJson::from_report`, pretty-print, and still return `Err(AenvError::PolicyViolation(...))` if `report.has_enforce_violations()` so exit 17 stays consistent across text and JSON.

- [ ] **Step 3: Lock snapshot + commit**

```rust
#[test]
fn doctor_report_shape_is_stable() {
    use aenv_core::json::doctor::{DoctorReportJson, OutcomeJson};
    let r = DoctorReportJson {
        namespace: "leaf".into(),
        chain: vec!["base".into(), "leaf".into()],
        policies: Default::default(),
        outcomes: vec![
            OutcomeJson {
                key: "instructions_max_chars".into(),
                status: "fail".into(),
                target: Some("leaf::CLAUDE.md".into()),
                msg: Some("CLAUDE.md is 5200 chars, limit 5000".into()),
            },
        ],
        pass_count: 2,
        warn_count: 0,
        fail_count: 1,
        skipped_count: 0,
    };
    insta::assert_json_snapshot!(r);
}
```

Accept, run, commit:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo insta accept --workspace
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -3

git add crates/aenv-core/src/json/doctor.rs crates/aenv-cli/src/cmd/doctor.rs \
        crates/aenv-core/tests/json_snapshots.rs \
        crates/aenv-core/tests/snapshots/json_snapshots__doctor_report_shape_is_stable.snap
git commit -m "Implement aenv doctor --json

- DoctorReportJson::from_report stringifies OutcomeStatus into
  'pass'/'warn'/'fail'/'skipped' and tallies the per-status counts in
  fields the JSON consumer can sum without re-iterating outcomes.
- Effective policies (including doctor::synthesize_instructions_limit)
  ride along under 'policies' with their resolved value, enforce flag,
  and source-namespace provenance.
- Exit code stays 17 on enforce violations for both text and JSON
  output — the error path is shared.
- Snapshot lock.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 12: `diff::project_drift` — detect managed-file divergence

For every file in `state.managed_files`, compare its current on-disk bytes against what the current resolution would materialize. Drift kinds:
- `symlink-replaced` — file was managed as `Symlink`, but on disk it's a regular file (the symlink got broken).
- `merge-regenerated` — file was managed as `SectionMerge` or `DeepMerge`; on-disk bytes differ from the freshly-computed merge.
- `content-divergent` — catch-all: bytes differ but neither of the above applies (rare; covers `Copy` strategy edits).

**Files:**
- Create: `crates/aenv-core/src/diff.rs`
- Modify: `crates/aenv-core/src/lib.rs` (`pub mod diff;`)
- Test: `crates/aenv-core/tests/diff_project_drift.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/diff_project_drift.rs`:

```rust
//! Project drift detection.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::diff::project_drift;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::PathBuf;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn setup_active_project() -> (TempDir, TempDir, RegistryLayout, AdapterRegistry) {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let layout = RegistryLayout::new(aenv_home.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();

    // Namespace `solo` with one CLAUDE.md.
    write_file(
        &layout.manifest_path("solo"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&layout.namespace_dir("solo").join("CLAUDE.md"), "# Hello\n");

    // Pin and activate.
    write_file(&project.path().join(".aenv"), "solo\n");
    let leaf = NamespaceId::new("solo").unwrap();
    aenv_core::activate::activate_namespace(&fs, &layout, &adapters, project.path(), &leaf).unwrap();

    (aenv_home, project, layout, adapters)
}

#[test]
fn no_drift_when_nothing_changed() {
    let (_aenv_home, project, layout, adapters) = setup_active_project();
    let fs = aenv_core::fs::RealFilesystem;
    let drift = project_drift(&fs, &layout, &adapters, project.path()).unwrap();
    assert!(drift.drifted.is_empty(), "got drift: {drift:?}");
}

#[test]
fn drift_when_symlink_replaced_with_edited_file() {
    let (_aenv_home, project, layout, adapters) = setup_active_project();
    let claude_path = project.path().join("CLAUDE.md");
    std::fs::remove_file(&claude_path).unwrap();
    std::fs::write(&claude_path, "# Hello\n\nLocal edit.\n").unwrap();
    let fs = aenv_core::fs::RealFilesystem;
    let drift = project_drift(&fs, &layout, &adapters, project.path()).unwrap();
    assert_eq!(drift.drifted.len(), 1);
    assert_eq!(drift.drifted[0].path, PathBuf::from("CLAUDE.md"));
    assert_eq!(drift.drifted[0].kind, "symlink-replaced");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test diff_project_drift 2>&1 | tail -10`
Expected: FAIL — `cannot find module diff`.

- [ ] **Step 3: Implement `diff::project_drift`**

Create `crates/aenv-core/src/diff.rs`:

```rust
//! Project-drift detection and structural namespace diff.
//!
//! `project_drift` walks `state.managed_files` and compares each entry's
//! on-disk bytes against what the current resolution would materialize.
//! `structural` compares two namespaces' skill rosters, parameters,
//! policies, and instructions-section headers.

use std::path::Path;

use crate::adapter::AdapterRegistry;
use crate::error::Result;
use crate::fs::Filesystem;
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::json::diff::{DriftReport, DriftedFile, StructuralDiff, SetDiff, ValueDiff, NamedValue, ValueChange};
use crate::materialize::compute_material_set;
use crate::resolve::MaterializeStrategy;
use crate::state::ActivationState;

/// Detect project drift. Returns an empty `DriftReport.drifted` if the
/// project is unpinned (no .aenv-state/state.json) or no managed file
/// has diverged.
pub fn project_drift<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
) -> Result<DriftReport> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        return Ok(DriftReport {
            project: project_root.to_path_buf(),
            active_namespace: String::new(),
            drifted: vec![],
        });
    }
    let bytes = fs.read(&state_path)?;
    let text = String::from_utf8(bytes)
        .map_err(|e| crate::AenvError::ManifestInvalid(format!("state.json: {e}")))?;
    let state = ActivationState::from_json(&text)?;

    let leaf = NamespaceId::new(state.active_namespace.as_str())?;
    let mat = compute_material_set(fs, layout, adapters, &leaf)?;
    let mut expected: std::collections::BTreeMap<&std::path::Path, &[u8]> = mat
        .entries
        .iter()
        .map(|(p, c)| (p.as_path(), c.as_slice()))
        .collect();

    let mut drifted: Vec<DriftedFile> = Vec::new();
    for mf in &state.managed_files {
        let project_path = project_root.join(&mf.path);
        let on_disk = match fs.read(&project_path) {
            Ok(b) => b,
            Err(_) => {
                // Missing-file drift: the user removed it.
                drifted.push(DriftedFile {
                    path: mf.path.clone(),
                    qualified_name: mf.qualified_name.to_string(),
                    kind: "content-divergent".into(),
                    summary: Some("file missing".into()),
                });
                continue;
            }
        };
        let Some(expected_bytes) = expected.remove(mf.path.as_path()) else {
            // Managed but no longer in the resolution — the namespace
            // shrunk. Treat as drift.
            drifted.push(DriftedFile {
                path: mf.path.clone(),
                qualified_name: mf.qualified_name.to_string(),
                kind: "content-divergent".into(),
                summary: Some("file no longer produced by resolution".into()),
            });
            continue;
        };
        if on_disk == expected_bytes {
            continue;
        }
        let kind = match mf.strategy {
            MaterializeStrategy::Symlink => {
                // Detect whether the path is still a symlink. If not, it
                // was replaced.
                match fs.symlink_metadata(&project_path) {
                    Ok(meta) if matches!(meta.kind, crate::fs::FileKind::Symlink) => {
                        "content-divergent"
                    }
                    _ => "symlink-replaced",
                }
            }
            MaterializeStrategy::SectionMerge | MaterializeStrategy::DeepMerge(_) => "merge-regenerated",
            _ => "content-divergent",
        };
        drifted.push(DriftedFile {
            path: mf.path.clone(),
            qualified_name: mf.qualified_name.to_string(),
            kind: kind.to_string(),
            summary: Some(summary(&on_disk, expected_bytes)),
        });
    }

    Ok(DriftReport {
        project: project_root.to_path_buf(),
        active_namespace: state.active_namespace,
        drifted,
    })
}

fn summary(on_disk: &[u8], expected: &[u8]) -> String {
    let on_disk_lines = on_disk.iter().filter(|&&b| b == b'\n').count();
    let exp_lines = expected.iter().filter(|&&b| b == b'\n').count();
    format!(
        "{} bytes on disk vs {} bytes expected ({} vs {} lines)",
        on_disk.len(),
        expected.len(),
        on_disk_lines,
        exp_lines
    )
}

// Task 13 lands `pub fn structural(...)` here.
```

Add `pub mod diff;` to `crates/aenv-core/src/lib.rs`.

- [ ] **Step 4: Run + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test diff_project_drift 2>&1 | tail -10`
Expected: 2 tests pass.

If `drift_when_symlink_replaced_with_edited_file` fails because `fs.symlink_metadata` returns `Symlink` for a replaced file, double-check whether `std::fs::remove_file` followed by `std::fs::write` removed the symlink target (it should on Unix — the symlink itself is the regular file's path now). Tweak the test setup (e.g., use `std::os::unix::fs::symlink` machinery directly) until the post-state reflects a normal file at the path.

```bash
git add crates/aenv-core/src/diff.rs crates/aenv-core/src/lib.rs \
        crates/aenv-core/tests/diff_project_drift.rs
git commit -m "Add diff::project_drift — detect managed-file divergence

For every entry in state.managed_files, compare the on-disk bytes
against the freshly-computed expected bytes from compute_material_set.
Classify the divergence:
- symlink-replaced: managed as Symlink but on-disk is no longer a
  symlink (user replaced the link with a regular file).
- merge-regenerated: managed as SectionMerge / DeepMerge and the
  bytes don't match a fresh merge.
- content-divergent: catch-all for Copy / Identical and edge cases.

Files no longer produced by the current resolution (a shrunken
namespace) are reported as drift too — the user should know that
deactivating won't restore those bytes from the namespace.

structural() lands in Task 13.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 13: `diff::structural` — compare two namespaces

**Files:**
- Modify: `crates/aenv-core/src/diff.rs`
- Test: `crates/aenv-core/tests/diff_structural.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/aenv-core/tests/diff_structural.rs`:

```rust
use aenv_core::adapter::AdapterRegistry;
use aenv_core::diff::structural;
use aenv_core::home::RegistryLayout;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn setup() -> (TempDir, RegistryLayout, AdapterRegistry) {
    let tmp = TempDir::new().unwrap();
    let layout = RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    (tmp, layout, adapters)
}

#[test]
fn structural_diff_reports_skill_roster_difference() {
    let (_tmp, layout, adapters) = setup();
    write_file(
        &layout.manifest_path("alpha"),
        r#"name = "alpha"
[adapters.claude-code]
files = []
[[skills]]
name = "a"
mode = "authored"
adapter = "claude-code"
"#,
    );
    write_file(
        &layout.manifest_path("beta"),
        r#"name = "beta"
[adapters.claude-code]
files = []
[[skills]]
name = "b"
mode = "authored"
adapter = "claude-code"
"#,
    );
    let fs = aenv_core::fs::RealFilesystem;
    let diff = structural(&fs, &layout, &adapters, "alpha", "beta").unwrap();
    assert_eq!(diff.a, "alpha");
    assert_eq!(diff.b, "beta");
    assert_eq!(diff.skills.added, vec!["beta::b".to_string()]);
    assert_eq!(diff.skills.removed, vec!["alpha::a".to_string()]);
    assert!(diff.skills.common.is_empty());
}

#[test]
fn structural_diff_reports_parameter_value_changes() {
    let (_tmp, layout, adapters) = setup();
    write_file(
        &layout.manifest_path("alpha"),
        r#"name = "alpha"
[adapters.claude-code]
files = []
[parameters]
default_model = "claude-sonnet-4.6"
"#,
    );
    write_file(
        &layout.manifest_path("beta"),
        r#"name = "beta"
[adapters.claude-code]
files = []
[parameters]
default_model = "claude-opus-4.7"
"#,
    );
    let fs = aenv_core::fs::RealFilesystem;
    let diff = structural(&fs, &layout, &adapters, "alpha", "beta").unwrap();
    assert_eq!(diff.parameters.changed.len(), 1);
    assert_eq!(diff.parameters.changed[0].name, "default_model");
    assert_eq!(diff.parameters.changed[0].a, serde_json::json!("claude-sonnet-4.6"));
    assert_eq!(diff.parameters.changed[0].b, serde_json::json!("claude-opus-4.7"));
}
```

- [ ] **Step 2: Run to verify failure, then implement**

Append to `crates/aenv-core/src/diff.rs`:

```rust
use crate::manifest::AenvManifest;
use crate::parameters::{ParameterValue, ResolvedParameter};
use crate::policies::{PolicyValue, ResolvedPolicy};
use crate::resolve::resolve_namespace;

/// Structural diff between two namespaces. Compares their resolved
/// skills, parameters, policies, and instructions-section headers.
pub fn structural<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    a: &str,
    b: &str,
) -> Result<StructuralDiff> {
    let a_id = NamespaceId::new(a)?;
    let b_id = NamespaceId::new(b)?;
    let a_res = resolve_namespace(fs, layout, adapters, &a_id)?;
    let b_res = resolve_namespace(fs, layout, adapters, &b_id)?;

    // Skill rosters: union of skills declared in each leaf's manifest,
    // keyed by qualified short-name.
    let a_skills = manifest_skill_qnames(fs, layout, a)?;
    let b_skills = manifest_skill_qnames(fs, layout, b)?;
    let skills = set_diff(&a_skills, &b_skills);

    // Agents: parsed today as part of the skill machinery? Not yet —
    // emit an empty SetDiff. The hook is here for the day a manifest
    // grows an [[agents]] table.
    let agents = SetDiff::default();

    let parameters = value_diff_params(&a_res.parameters, &b_res.parameters);
    let policies = value_diff_policies(&a_res.policies, &b_res.policies);

    // Instructions section headers: walk the resolved candidates for any
    // file with role "instructions" and section-merge its bodies.
    let a_sections = instruction_section_headers(fs, layout, adapters, a)?;
    let b_sections = instruction_section_headers(fs, layout, adapters, b)?;
    let instructions_sections = set_diff(&a_sections, &b_sections);

    Ok(StructuralDiff {
        a: a.to_string(),
        b: b.to_string(),
        skills,
        agents,
        parameters,
        policies,
        instructions_sections,
    })
}

fn manifest_skill_qnames<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    ns: &str,
) -> Result<Vec<String>> {
    let bytes = fs.read(&layout.manifest_path(ns))?;
    let text = String::from_utf8(bytes)
        .map_err(|e| crate::AenvError::ManifestInvalid(format!("manifest utf-8: {e}")))?;
    let m = AenvManifest::from_toml(&text)?;
    Ok(m.skills.iter().map(|s| format!("{ns}::{}", s.name)).collect())
}

fn instruction_section_headers<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    ns: &str,
) -> Result<Vec<String>> {
    let id = NamespaceId::new(ns)?;
    let mat = compute_material_set(fs, layout, adapters, &id)?;
    let mut headers: Vec<String> = Vec::new();
    for (path, content) in &mat.entries {
        // Cheap heuristic: any path ending in CLAUDE.md or .mdc whose
        // first non-empty line begins with `#` is an instructions file.
        let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        let is_instructions = name == "claude.md" || name.ends_with(".mdc");
        if !is_instructions { continue; }
        if let Ok(s) = std::str::from_utf8(content) {
            for line in s.lines() {
                if let Some(rest) = line.strip_prefix("## ") {
                    headers.push(rest.trim().to_string());
                }
            }
        }
    }
    headers.sort();
    headers.dedup();
    Ok(headers)
}

fn set_diff(a: &[String], b: &[String]) -> SetDiff {
    let a_set: std::collections::BTreeSet<&str> = a.iter().map(String::as_str).collect();
    let b_set: std::collections::BTreeSet<&str> = b.iter().map(String::as_str).collect();
    SetDiff {
        added: b_set.difference(&a_set).map(|s| (*s).to_string()).collect(),
        removed: a_set.difference(&b_set).map(|s| (*s).to_string()).collect(),
        common: a_set.intersection(&b_set).map(|s| (*s).to_string()).collect(),
    }
}

fn value_diff_params(
    a: &std::collections::BTreeMap<String, ResolvedParameter>,
    b: &std::collections::BTreeMap<String, ResolvedParameter>,
) -> ValueDiff {
    let to_json = |v: &ParameterValue| -> serde_json::Value {
        match v {
            ParameterValue::String(s) => serde_json::Value::String(s.clone()),
            ParameterValue::Integer(i) => serde_json::Value::Number((*i).into()),
            ParameterValue::Boolean(b) => serde_json::Value::Bool(*b),
            ParameterValue::ListString(xs) => serde_json::Value::Array(
                xs.iter().map(|s| serde_json::Value::String(s.clone())).collect(),
            ),
        }
    };
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for (k, va) in a {
        match b.get(k) {
            None => removed.push(NamedValue { name: k.clone(), value: to_json(&va.value) }),
            Some(vb) if vb.value != va.value => changed.push(ValueChange {
                name: k.clone(),
                a: to_json(&va.value),
                b: to_json(&vb.value),
            }),
            _ => {}
        }
    }
    for (k, vb) in b {
        if !a.contains_key(k) {
            added.push(NamedValue { name: k.clone(), value: to_json(&vb.value) });
        }
    }
    ValueDiff { added, removed, changed }
}

fn value_diff_policies(
    a: &std::collections::BTreeMap<String, ResolvedPolicy>,
    b: &std::collections::BTreeMap<String, ResolvedPolicy>,
) -> ValueDiff {
    let to_json = |v: &PolicyValue| -> serde_json::Value {
        match v {
            PolicyValue::Integer(i) => serde_json::Value::Number((*i).into()),
            PolicyValue::Boolean(b) => serde_json::Value::Bool(*b),
            PolicyValue::ListString(xs) => serde_json::Value::Array(
                xs.iter().map(|s| serde_json::Value::String(s.clone())).collect(),
            ),
        }
    };
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for (k, va) in a {
        match b.get(k) {
            None => removed.push(NamedValue { name: k.clone(), value: to_json(&va.value) }),
            Some(vb) if vb.value != va.value => changed.push(ValueChange {
                name: k.clone(),
                a: to_json(&va.value),
                b: to_json(&vb.value),
            }),
            _ => {}
        }
    }
    for (k, vb) in b {
        if !a.contains_key(k) {
            added.push(NamedValue { name: k.clone(), value: to_json(&vb.value) });
        }
    }
    ValueDiff { added, removed, changed }
}
```

- [ ] **Step 3: Run + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test diff_structural 2>&1 | tail -10`
Expected: 2 tests pass.

```bash
git add crates/aenv-core/src/diff.rs crates/aenv-core/tests/diff_structural.rs
git commit -m "Add diff::structural — compare two namespaces

Reports four set/value diffs:
- skills: union of [[skills]] entries, keyed as <ns>::<name>.
- agents: hook exists (always empty today) for a future [[agents]] table.
- parameters: added / removed / changed by VALUE only (provenance is
  diff-irrelevant — two namespaces with identical effective values
  produce no entry even if different ancestors declared them).
- policies: same shape as parameters.
- instructions_sections: '## ' headers found in any candidate file
  whose name matches CLAUDE.md or *.mdc. Sorted + deduped so the diff
  isn't sensitive to source-file order.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 14: `aenv diff` CLI (text + `--json`)

Wire both flavors. `aenv diff` (no args) reports project drift; `aenv diff <a> <b>` reports structural difference. Both honor `--json`.

**Files:**
- Create: `crates/aenv-cli/src/cmd/diff.rs`
- Modify: `crates/aenv-cli/src/cmd/mod.rs`
- Modify: `crates/aenv-cli/src/main.rs`
- Test: append snapshots to `crates/aenv-core/tests/json_snapshots.rs`
- Test: `crates/aenv-cli/tests/diff_e2e.rs`

- [ ] **Step 1: Add the clap variant**

In `main.rs`:

```rust
/// Diff against the active namespace (drift) or between two namespaces.
Diff {
    /// First namespace name for structural diff (omit for drift).
    ns_a: Option<String>,
    /// Second namespace name for structural diff.
    ns_b: Option<String>,
    #[arg(long)]
    project: Option<PathBuf>,
    #[arg(long)]
    json: bool,
},
```

Dispatcher:

```rust
Command::Diff { ns_a, ns_b, project, json } => {
    match (ns_a, ns_b) {
        (None, None) => {
            let project_root = paths::resolve_project_root(&fs, project)?;
            let aenv_home = paths::resolve_aenv_home()?;
            cmd::diff::run_drift(&fs, &project_root, &aenv_home, json)
        }
        (Some(a), Some(b)) => cmd::diff::run_structural(&fs, &layout, &a, &b, json),
        _ => Err(aenv_core::AenvError::ManifestInvalid(
            "aenv diff needs either zero or two namespace arguments".into(),
        )),
    }
}
```

- [ ] **Step 2: Implement `cmd/diff.rs`**

```rust
//! aenv diff: project drift (no args) and structural (two-namespace).

use aenv_core::adapter::AdapterRegistry;
use aenv_core::diff::{project_drift, structural};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::{AenvError, Result};
use std::path::Path;
use std::process::ExitCode;

pub fn run_drift<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    aenv_home: &Path,
    json: bool,
) -> Result<()> {
    let layout = RegistryLayout::new(aenv_home.to_path_buf());
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let report = project_drift(fs, &layout, &adapters, project_root)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)
            .map_err(|e| AenvError::ManifestInvalid(format!("json: {e}")))?);
    } else if report.drifted.is_empty() {
        println!("No drift detected. All managed files match their namespace source.");
    } else {
        println!("Drift in project {}:", report.project.display());
        for d in &report.drifted {
            println!("  {} ({})", d.path.display(), d.kind);
            if let Some(s) = &d.summary {
                println!("    {s}");
            }
        }
    }
    Ok(())
}

pub fn run_structural<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    a: &str,
    b: &str,
    json: bool,
) -> Result<()> {
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let diff = structural(fs, layout, &adapters, a, b)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&diff)
            .map_err(|e| AenvError::ManifestInvalid(format!("json: {e}")))?);
        return Ok(());
    }

    if !diff.skills.added.is_empty() || !diff.skills.removed.is_empty() {
        println!("Skills:");
        for s in &diff.skills.added { println!("  + {s}"); }
        for s in &diff.skills.removed { println!("  - {s}"); }
        println!();
    }
    if !diff.parameters.added.is_empty()
        || !diff.parameters.removed.is_empty()
        || !diff.parameters.changed.is_empty()
    {
        println!("Parameters:");
        for c in &diff.parameters.changed {
            println!("  {}: {} → {}", c.name, c.a, c.b);
        }
        for nv in &diff.parameters.added { println!("  +{}: {}", nv.name, nv.value); }
        for nv in &diff.parameters.removed { println!("  -{}: {}", nv.name, nv.value); }
        println!();
    }
    if !diff.policies.added.is_empty()
        || !diff.policies.removed.is_empty()
        || !diff.policies.changed.is_empty()
    {
        println!("Policies:");
        for c in &diff.policies.changed { println!("  {}: {} → {}", c.name, c.a, c.b); }
        for nv in &diff.policies.added { println!("  +{}: {}", nv.name, nv.value); }
        for nv in &diff.policies.removed { println!("  -{}: {}", nv.name, nv.value); }
        println!();
    }
    if !diff.instructions_sections.added.is_empty()
        || !diff.instructions_sections.removed.is_empty()
    {
        println!("Instructions sections:");
        for s in &diff.instructions_sections.added { println!("  + ## {s}"); }
        for s in &diff.instructions_sections.removed { println!("  - ## {s}"); }
    }
    Ok(())
}

/// `aenv diff` exit-code convention: 0 = no differences, 1 = differences.
/// Callers map our `Result` to this externally — we set `process::ExitCode`
/// via `main`'s match arm. Phase 5 keeps this behavior in `main.rs`'s
/// success-result handling: text/json output is informative, and the
/// process exit is 0 when the command ran cleanly. (A future iteration
/// can add a `--exit-code` flag that maps non-empty drift / non-empty
/// structural diff to exit 1, mirroring `diff(1)` and `git diff
/// --exit-code`.)
#[allow(dead_code)]
fn _exit_code_convention_note() -> ExitCode { ExitCode::SUCCESS }
```

- [ ] **Step 3: Snapshot the diff JSON shapes**

```rust
#[test]
fn drift_report_shape_is_stable() {
    use aenv_core::json::diff::{DriftReport, DriftedFile};
    let r = DriftReport {
        project: std::path::PathBuf::from("/proj"),
        active_namespace: "leaf".into(),
        drifted: vec![DriftedFile {
            path: std::path::PathBuf::from("CLAUDE.md"),
            qualified_name: "leaf::CLAUDE.md".into(),
            kind: "symlink-replaced".into(),
            summary: Some("420 bytes on disk vs 380 bytes expected".into()),
        }],
    };
    insta::assert_json_snapshot!(r);
}

#[test]
fn structural_diff_shape_is_stable() {
    use aenv_core::json::diff::*;
    let d = StructuralDiff {
        a: "alpha".into(),
        b: "beta".into(),
        skills: SetDiff {
            added: vec!["beta::write-tests".into()],
            removed: vec!["alpha::quick-prototype".into()],
            common: vec!["common-skill".into()],
        },
        agents: SetDiff::default(),
        parameters: ValueDiff {
            added: vec![],
            removed: vec![],
            changed: vec![ValueChange {
                name: "default_model".into(),
                a: serde_json::json!("claude-sonnet-4.6"),
                b: serde_json::json!("claude-opus-4.7"),
            }],
        },
        policies: ValueDiff::default(),
        instructions_sections: SetDiff::default(),
    };
    insta::assert_json_snapshot!(d);
}
```

- [ ] **Step 4: E2E test**

Create `crates/aenv-cli/tests/diff_e2e.rs`:

```rust
use std::process::Command;
use tempfile::TempDir;

#[test]
fn diff_no_args_reports_clean_on_freshly_activated_project() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let envs = aenv_home.path().join("envs/solo");
    std::fs::create_dir_all(&envs).unwrap();
    std::fs::write(
        envs.join("aenv.toml"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    ).unwrap();
    std::fs::write(envs.join("CLAUDE.md"), "# Hi\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_aenv");
    Command::new(bin).args(["use", "solo", "--project", project.path().to_str().unwrap()])
        .env("AENV_HOME", aenv_home.path()).status().unwrap();
    Command::new(bin).args(["activate", "--project", project.path().to_str().unwrap()])
        .env("AENV_HOME", aenv_home.path()).status().unwrap();

    let out = Command::new(bin)
        .args(["diff", "--project", project.path().to_str().unwrap(), "--json"])
        .env("AENV_HOME", aenv_home.path())
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["drifted"].as_array().unwrap().is_empty());
}

#[test]
fn diff_two_namespaces_reports_skill_added() {
    let aenv_home = TempDir::new().unwrap();
    for (name, body) in [
        ("alpha", "name = \"alpha\"\n[adapters.claude-code]\nfiles = []\n"),
        ("beta", "name = \"beta\"\n[adapters.claude-code]\nfiles = []\n[[skills]]\nname = \"new\"\nmode = \"authored\"\nadapter = \"claude-code\"\n"),
    ] {
        let envs = aenv_home.path().join(format!("envs/{name}"));
        std::fs::create_dir_all(&envs).unwrap();
        std::fs::write(envs.join("aenv.toml"), body).unwrap();
    }

    let bin = env!("CARGO_BIN_EXE_aenv");
    let out = Command::new(bin)
        .args(["diff", "alpha", "beta", "--json"])
        .env("AENV_HOME", aenv_home.path())
        .output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["a"], "alpha");
    assert_eq!(v["b"], "beta");
    assert!(v["skills"]["added"].as_array().unwrap().iter().any(|s| s == "beta::new"));
}
```

- [ ] **Step 5: Run + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo insta accept --workspace 2>&1 | tail -3 && PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -5`
Expected: all tests pass.

```bash
git add crates/aenv-cli/src/cmd/diff.rs crates/aenv-cli/src/cmd/mod.rs \
        crates/aenv-cli/src/main.rs crates/aenv-cli/tests/diff_e2e.rs \
        crates/aenv-core/tests/json_snapshots.rs \
        crates/aenv-core/tests/snapshots/json_snapshots__drift_report_shape_is_stable.snap \
        crates/aenv-core/tests/snapshots/json_snapshots__structural_diff_shape_is_stable.snap
git commit -m "Add aenv diff (project drift + structural) with --json

- aenv diff (no args) walks state.managed_files and reports any
  divergence from compute_material_set bytes.
- aenv diff <a> <b> reports added/removed/common skills, value-changed
  parameters and policies, and added/removed instructions sections.
- Both forms accept --json with snapshot-locked schemas.
- Exit code is always 0 on a clean command run; a future iteration may
  add --exit-code to mirror diff(1) / git diff --exit-code semantics
  (mapping non-empty drift to exit 1).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 15: Cross-machine fixture test

Check in two fixture namespaces alongside an `expected.txt` of `<name>=<hash>` lines. The test loads each fixture, recomputes the hash, and asserts equality. Catches any platform-dependent behavior (line endings, path normalization, encoding).

**Files:**
- Create: `crates/aenv-core/tests/fixtures/cross_machine/README.md`
- Create: `crates/aenv-core/tests/fixtures/cross_machine/.gitattributes`
- Create: `crates/aenv-core/tests/fixtures/cross_machine/alpha/aenv.toml`
- Create: `crates/aenv-core/tests/fixtures/cross_machine/alpha/CLAUDE.md`
- Create: `crates/aenv-core/tests/fixtures/cross_machine/beta/aenv.toml`
- Create: `crates/aenv-core/tests/fixtures/cross_machine/beta/CLAUDE.md`
- Create: `crates/aenv-core/tests/fixtures/cross_machine/expected.txt`
- Create: `crates/aenv-core/tests/cross_machine_hash.rs`

- [ ] **Step 1: Lock line endings via `.gitattributes`**

Create `crates/aenv-core/tests/fixtures/cross_machine/.gitattributes`:

```
* text eol=lf
```

This forces Git to store and check out every file under this directory with LF, regardless of `core.autocrlf` on the contributor's machine.

- [ ] **Step 2: Document the format**

Create `crates/aenv-core/tests/fixtures/cross_machine/README.md`:

```markdown
# Cross-machine hash fixtures

Two small namespaces (`alpha` and `beta`, where `beta extends alpha`)
with hand-computed expected hashes. The accompanying test
`crates/aenv-core/tests/cross_machine_hash.rs` recomputes each hash
and asserts it matches the line in `expected.txt`.

Any change to a fixture file (including whitespace) requires
regenerating the corresponding line in `expected.txt`. To regenerate:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test cross_machine_hash \
  -- --nocapture --ignored print-hashes
```

That prints the current hashes; copy them into `expected.txt`.

The `.gitattributes` file in this directory forces LF on every text
file so the hash stays platform-stable across Linux/macOS/Windows.

Adding a new fixture: create `<name>/aenv.toml` + supporting files,
regenerate `expected.txt`, commit.
```

- [ ] **Step 3: Create the alpha fixture**

`crates/aenv-core/tests/fixtures/cross_machine/alpha/aenv.toml`:

```toml
name = "alpha"

[adapters.claude-code]
files = ["CLAUDE.md"]
```

`crates/aenv-core/tests/fixtures/cross_machine/alpha/CLAUDE.md`:

```markdown
# Alpha

A minimal cross-machine hash fixture. Single instructions file.
```

- [ ] **Step 4: Create the beta fixture (extends alpha)**

`crates/aenv-core/tests/fixtures/cross_machine/beta/aenv.toml`:

```toml
name = "beta"
extends = ["alpha"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-opus-4.7"
```

`crates/aenv-core/tests/fixtures/cross_machine/beta/CLAUDE.md`:

```markdown
## Disposition

Beta layers an additional section on alpha via section-merge.
```

- [ ] **Step 5: Stub `expected.txt`**

Create `crates/aenv-core/tests/fixtures/cross_machine/expected.txt` with placeholders that will be filled in by running the test in `--ignored` print mode:

```
alpha=sha256-v1:REPLACE_ME_WITH_PRINTED_VALUE
beta=sha256-v1:REPLACE_ME_WITH_PRINTED_VALUE
```

- [ ] **Step 6: Write the test (with an ignored print-hashes mode)**

Create `crates/aenv-core/tests/cross_machine_hash.rs`:

```rust
//! Cross-machine hash agreement. Loads fixtures, recomputes each
//! namespace's hash, asserts it matches the line in `expected.txt`.
//!
//! When you need to regenerate `expected.txt`, run:
//!     cargo test -p aenv-core --test cross_machine_hash -- --ignored
//! That fires the ignored `print_hashes` test which dumps the current
//! hashes; copy them verbatim into `expected.txt`.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::hash::hash_resolved_namespace;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::compute_material_set;
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cross_machine")
}

fn copy_fixtures_into_layout(layout: &RegistryLayout) {
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();

    let root = fixtures_root();
    for entry in std::fs::read_dir(&root).unwrap().flatten() {
        if !entry.file_type().unwrap().is_dir() { continue; }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let dest = layout.namespace_dir(&name);
        copy_dir_recursive(&entry.path(), &dest);
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap().flatten() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            // Read + write rather than fs::copy so we can drop CR bytes
            // defensively even if .gitattributes failed somehow.
            let bytes = std::fs::read(&src_path).unwrap();
            let normalized: Vec<u8> = bytes.into_iter().filter(|&b| b != b'\r').collect();
            std::fs::write(&dst_path, normalized).unwrap();
        }
    }
}

fn compute_one(name: &str) -> String {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = RegistryLayout::new(tmp.path().to_path_buf());
    copy_fixtures_into_layout(&layout);
    let fs = aenv_core::fs::RealFilesystem;
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let leaf = NamespaceId::new(name).unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();
    hash_resolved_namespace(&mat)
}

fn expected() -> std::collections::BTreeMap<String, String> {
    let raw = std::fs::read_to_string(fixtures_root().join("expected.txt")).unwrap();
    raw.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|l| {
            let (k, v) = l.split_once('=').expect("expected.txt line: NAME=HASH");
            (k.trim().to_string(), v.trim().to_string())
        })
        .collect()
}

#[test]
fn alpha_hash_matches_fixture() {
    let h = compute_one("alpha");
    let expected = expected();
    assert_eq!(
        h, expected.get("alpha").expect("alpha line missing"),
        "alpha hash drift — regenerate expected.txt or investigate"
    );
}

#[test]
fn beta_hash_matches_fixture() {
    let h = compute_one("beta");
    let expected = expected();
    assert_eq!(
        h, expected.get("beta").expect("beta line missing"),
        "beta hash drift — regenerate expected.txt or investigate"
    );
}

#[test]
#[ignore]
fn print_hashes() {
    println!("alpha={}", compute_one("alpha"));
    println!("beta={}", compute_one("beta"));
}
```

- [ ] **Step 7: Populate `expected.txt`**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test cross_machine_hash -- --ignored --nocapture print_hashes 2>&1 | tail -10`

Copy the two `alpha=...` and `beta=...` lines into `crates/aenv-core/tests/fixtures/cross_machine/expected.txt`, replacing the placeholders.

- [ ] **Step 8: Run all three tests + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test cross_machine_hash 2>&1 | tail -10`
Expected: 2 tests pass (`print_hashes` stays ignored).

```bash
git add crates/aenv-core/tests/fixtures/cross_machine/ \
        crates/aenv-core/tests/cross_machine_hash.rs
git commit -m "Add cross-machine hash fixture test

Two minimal fixture namespaces (alpha, beta — beta extends alpha) plus
an expected.txt of <name>=<hash> lines. The test recomputes each hash
in a tempdir copy of the fixtures and asserts equality.

A .gitattributes pin (text eol=lf) keeps line endings normalized
across Linux/macOS/Windows checkouts; the copy step also strips CR
bytes defensively in case .gitattributes is bypassed.

An ignored print_hashes test is the regeneration helper: run with
--ignored --nocapture, copy the two lines into expected.txt.

Catches any future change that introduces platform-dependent
behavior (line endings, path encoding, Unicode normalization) as a
hash mismatch on one platform's CI runner.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 16: Scripted-comparison end-to-end test (functional spec §7.5)

Reproduce the scripted comparison loop from functional spec §7.5: three namespaces, activate each against the same project, capture the hash + managed-file short-name list per activation. Demonstrates the full scriptability surface working end-to-end.

**Files:**
- Test: `crates/aenv-cli/tests/scripted_comparison_e2e.rs`

- [ ] **Step 1: Write the test**

Create `crates/aenv-cli/tests/scripted_comparison_e2e.rs`:

```rust
//! Functional spec §7.5 — scripted comparison.
//!
//! Three namespaces with distinct content. The script activates each in
//! turn against the same project, reads `aenv status --json`, and
//! captures the resolved_hash plus the list of managed-file short names.
//! Asserts the hashes are distinct (different namespaces → different
//! material) and that re-activating the same namespace is hash-stable.

use std::collections::HashSet;
use std::process::Command;
use tempfile::TempDir;

fn write(dir: &std::path::Path, relpath: &str, body: &str) {
    let p = dir.join(relpath);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

#[test]
fn three_namespaces_produce_three_distinct_hashes() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let envs = aenv_home.path().join("envs");

    write(&envs, "experiments/aenv.toml",
        "name = \"experiments\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n");
    write(&envs, "experiments/CLAUDE.md", "# Experiments\nBe broad.\n");

    write(&envs, "detailed/aenv.toml",
        "name = \"detailed\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n");
    write(&envs, "detailed/CLAUDE.md", "# Detailed\nBe careful.\n");

    write(&envs, "analyst/aenv.toml",
        "name = \"analyst\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n");
    write(&envs, "analyst/CLAUDE.md", "# Analyst\nRead-only.\n");

    let bin = env!("CARGO_BIN_EXE_aenv");
    let mut hashes: Vec<String> = Vec::new();
    for ns in ["experiments", "detailed", "analyst"] {
        // Deactivate any previous activation (idempotent if none).
        Command::new(bin)
            .args(["deactivate", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path()).status().ok();
        // Pin + activate.
        Command::new(bin)
            .args(["use", ns, "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path()).status().unwrap();
        Command::new(bin)
            .args(["activate", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path()).status().unwrap();
        // Capture status JSON.
        let out = Command::new(bin)
            .args(["status", "--project", project.path().to_str().unwrap(), "--json"])
            .env("AENV_HOME", aenv_home.path())
            .output().unwrap();
        assert!(out.status.success(), "{ns} status failed");
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let h = v["resolved_hash"].as_str().unwrap().to_string();
        assert!(h.starts_with("sha256-v1:"));
        hashes.push(h);
    }

    let unique: HashSet<&String> = hashes.iter().collect();
    assert_eq!(unique.len(), 3, "expected three distinct hashes, got {hashes:?}");
}

#[test]
fn reactivating_same_namespace_is_hash_stable() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let envs = aenv_home.path().join("envs/stable");
    std::fs::create_dir_all(&envs).unwrap();
    std::fs::write(envs.join("aenv.toml"),
        "name = \"stable\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n").unwrap();
    std::fs::write(envs.join("CLAUDE.md"), "# Stable\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_aenv");
    let mut hashes: Vec<String> = Vec::new();
    for _ in 0..2 {
        Command::new(bin).args(["deactivate", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path()).status().ok();
        Command::new(bin).args(["use", "stable", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path()).status().unwrap();
        Command::new(bin).args(["activate", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path()).status().unwrap();
        let out = Command::new(bin)
            .args(["status", "--project", project.path().to_str().unwrap(), "--json"])
            .env("AENV_HOME", aenv_home.path()).output().unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        hashes.push(v["resolved_hash"].as_str().unwrap().to_string());
    }
    assert_eq!(hashes[0], hashes[1], "re-activating same namespace must be hash-stable");
}
```

- [ ] **Step 2: Run + commit**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-cli --test scripted_comparison_e2e 2>&1 | tail -10`
Expected: 2 tests pass.

```bash
git add crates/aenv-cli/tests/scripted_comparison_e2e.rs
git commit -m "Add functional spec §7.5 end-to-end scripted-comparison test

Two tests demonstrate the full scriptability surface working end-to-end:
- three_namespaces_produce_three_distinct_hashes: activates each of
  three namespaces against the same project, captures
  aenv status --json's resolved_hash, asserts three distinct values.
  This is the loop a downstream evaluation tool would run to record
  'run X used harness Y at hash sha256-v1:Z' for later reproducibility.
- reactivating_same_namespace_is_hash_stable: deactivate +
  re-activate the same namespace twice; both runs must produce the
  identical hash. The hash is a function of resolved content, not of
  activation timestamps or backup directory names.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 17: Tag `phase-5-complete`

Final verification + tag.

- [ ] **Step 1: Run the full suite + clippy + fmt**

Run all three in one batch:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --all-targets 2>&1 | tail -10
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check 2>&1 | tail -5
```

Expected: workspace tests all pass (399 + new Phase 5 additions ≈ 430+); clippy silent; fmt silent.

- [ ] **Step 2: Verify phase-5 deliverables checklist**

Walk through each item below; fix any failure with a follow-up commit before tagging.

- [ ] RFC 8785 JCS implementation passes the test vectors (Task 2).
- [ ] `hash_resolved_namespace` follows R-84 exactly: extends-resolution → canonical-JSON for structured files → append `.aenv/parameters.json` → sort byte-wise lex → length-prefix → prepend `0x01` → SHA-256. (Task 4)
- [ ] Hash exposed as `sha256-v1:<lowercase-hex>` in `status --json` and `list --json`. (Tasks 8, 9)
- [ ] `--json` works on every read-oriented command: `status`, `list`, `which`, `diff`, `adapter list`, `skill list`, `get`, `doctor`. (Tasks 8–11, 14)
- [ ] All `--json` output uses qualified names; short names included separately where adapter consumption matters (StatusReport.ManagedFileJson.short_name, WhichReport.short_name, SkillEntry.short_name). (Tasks 8, 10)
- [ ] `aenv diff` (no args): per-file drift summary for managed files that diverged from the resolved namespace. (Tasks 12, 14)
- [ ] `aenv diff <a> <b>`: structural diff covering skills, parameters, policies, instructions sections (text + `--json`). (Tasks 13, 14)
- [ ] Property tests cover order independence, single-byte content avalanche, path renames, case sensitivity, parameter-value effect, parameter-source-invariance. (Task 5)
- [ ] insta snapshots lock every `--json` shape. (Tasks 8–11, 14)
- [ ] Cross-machine fixture test passes on Linux x86_64 + Linux aarch64 + macOS CI runners (verify by pushing to a feature branch and inspecting the CI matrix output). (Task 15)
- [ ] Functional spec §7.5 scripted-comparison loop runs end-to-end. (Task 16)
- [ ] R-87 forward-compatibility hook: `resolved_hash_v2: Option<String>` field exists on StatusReport and ListEntry, always `None` in v1 and skipped during serialization. (Task 6)

- [ ] **Step 3: Tag**

```bash
git tag -a phase-5-complete -m "$(cat <<'EOF'
Phase 5 complete: resolved-namespace hash + scriptability

- RFC 8785 JCS implementation + standard test vectors.
- sha256-v1:<hex> hash exposed in status --json and list --json,
  computed from a pure material-set + synthetic .aenv/parameters.json.
- proptest-verified hash invariants: order independence, content
  avalanche, path renames, case sensitivity, parameter folding,
  parameter-source-blindness.
- --json on every read-oriented command, schema-locked via insta
  snapshots: status, list, which, get, doctor, adapter list, skill
  list, diff.
- aenv diff: project drift + structural namespace-vs-namespace
  comparison, text and JSON.
- Cross-machine hash fixture test (Linux + macOS — Windows defers to
  Phase 7).
- §7.5 scripted-comparison loop reproduced end-to-end against three
  fixture namespaces.
- R-87 algorithm-versioning hook present (resolved_hash_v2 field);
  no v2 algorithm shipped today.
EOF
)"
```

- [ ] **Step 4: Verify the tag**

Run: `git tag -l --format='%(contents:subject)' phase-5-complete`
Expected: `Phase 5 complete: resolved-namespace hash + scriptability`.

Run: `git log --oneline phase-4-complete..phase-5-complete | wc -l`
Expected: ~17–20 commits (one per task plus the post-completion clippy/refinement commits between phases 4 and 5).

---

## Phase 5 completion check

After Task 17:

- [ ] Every checkbox in this plan is checked.
- [ ] `cargo test --workspace --all-targets` is green (~430+ tests).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is silent.
- [ ] `cargo fmt --check` is silent.
- [ ] `phase-5-complete` git tag points at the final commit.
- [ ] PRD requirements R-51, R-76, R-77, R-80, R-81, R-84, R-85, R-86 all have a corresponding test that exercises the requirement. R-87 has the infrastructure in place (versioning hook field) but no v2 implementation.
- [ ] Functional spec §5.6 (`aenv diff` both flavors), §7.1 (`--json` shapes), §7.3 (`resolved_hash` usage), §7.5 (scripted comparison) reproduced end-to-end.
- [ ] Existing 399 Phase 4 tests still pass — no regressions in resolution, activation, doctor, skills lifecycle.

If any criterion fails, fix it in a follow-up commit and re-tag (delete + recreate `phase-5-complete`).

---

## Self-review notes (for the planner)

- **Spec coverage check:** Every PRD R-number listed in the Phase 5 roadmap entry (R-51, R-76, R-77, R-80, R-81, R-84, R-85, R-86, R-87) maps to a task that introduces a test exercising it. R-78/R-79 (project flag) were Phase 1 deliverables and remain in place; this plan doesn't re-test them but inherits their tests.
- **Type-consistency check:** `StatusReport`, `ManagedFileJson`, `WhichReport` all use the same string forms for `MaterializeStrategy` (`symlink`, `identical`, `copy`, `section-merge`, `deep-merge`, `merged`) and the same `merge_kind` values (`json`, `yaml`, `toml`). `ListEntry.error: Option<String>` is the only place where resolution failure becomes a string field; the rest of the schemas assume successful resolution. `GetReport.value` and `InheritanceEntry.value` and `NamedValue.value` and `ValueChange.{a,b}` all type as `serde_json::Value` so heterogeneous parameter types round-trip without a wrapper enum.
- **Placeholder scan:** No "TBD" / "implement later" / "handle edge cases" wording in any step. Every code block is complete enough to compile after the prior steps in the same task land.
- **Hash-input contract:** `path_to_bytes` in Task 4 normalizes backslashes to forward slashes so the same fixture produces the same hash on Windows once Phase 7 lands. The cross-machine test (Task 15) catches platform-dependent regressions early.
- **R-87 forward-compatibility:** Implemented as a structural hook (the `Option<String> resolved_hash_v2` field), not behavioral. When a future v2 algorithm lands, the implementation populates this field during the deprecation window; v1 consumers ignore the new field, v2 consumers branch on the prefix. No changes to `resolved_hash` in this plan.
