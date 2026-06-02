# Plan: `aenv vendor` — copy non-skill content from a git source (issue #2)

## Context

aenv imports *skills* cleanly (`aenv skill import`, and `import-all` per #1) because
skills have a known shape (`SKILL.md` + frontmatter, materialized at
`.claude/skills/<name>/`). For everything else in a config/plugin repo — agents,
slash commands, reference docs, settings fragments — there is no verb: the user
hand-`git clone`s, `cp`s files into `~/.aenv/envs/<ns>/.claude/…`, and hand-edits
`files = [...]`. This adds `aenv vendor` to do that copy + manifest-update + record
provenance in one command.

**Key property:** vendored files are just *authored content* in the namespace tree
declared under an adapter's `files = [...]`. The resolver, materialization, and
`state.json` schema are **unchanged** — vendoring is a manifest-authoring command,
not an activation-time one.

## Decisions (recommended, from the issue)

- **Schema: a new top-level `[[vendored]]` array-of-tables** (option A), not a
  `mode = "vendored"` overload of `[[skills]]`. Keeps `[[skills]]` meaning
  "SKILL.md thing" and gives vendored content its own provenance lane. Confirm
  before building.
- **Provenance recorded in the manifest** (`[[vendored]]`), not in `state.json`
  (vendored files aren't activation state). No `state.json` schema bump.

## Reuse inventory (call, don't reimplement)

- `aenv_core::skills::source::SourceKind::parse` + `skills::git::git_clone` +
  `skills::cache::{skill_cache_path, source_hash}` — same fetch/cache path as
  `skill import`.
- **Adapter-prefix inference** — extract the `.codex/ → codex, else claude-code`
  logic from `global_snapshot.rs` (~lines 497–520) into a shared helper
  `adapter_for_path(rel) -> &str`, reused by both snapshot and vendor.
- **Sorted+deduped `files` append** — mirror
  `namespace::create_namespace_from_project` (`files.sort(); files.dedup();`).
- `SkillProvenance` (`state.rs`) as the shape precedent for `VendoredProvenance`.
- `AenvManifest::{from_toml,to_toml}` + the two-stage parser (`manifest.rs`).
- `walkdir::WalkDir` for the walk-and-copy.

## Schema additions (`crates/aenv-core/src/manifest.rs`)

```rust
/// One `[[vendored]]` entry: provenance for a non-skill subtree/file copied
/// into the namespace tree by `aenv vendor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VendoredDecl {
    pub source: String,                 // git URL or local path
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,           // resolved SHA for git sources
    pub src_path: String,               // subtree/file in the source
    pub dest: String,                   // namespace-relative destination (--as)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,             // expanded literal dest files (for drift/update)
}
```

- Add `vendored: Vec<VendoredDecl>` to `AenvManifest` (after `skills`, before
  `lifecycle`) with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`,
  and the matching `#[serde(default)] vendored` field on the `Raw` parse struct.
- Add a validation stage: each `dest` is a relative path (reuse
  `validate_relative_path`); `src_path` non-empty.
- **Round-trip risk to verify:** `[[vendored]]` (array-of-tables) must serialize
  after scalar/`[[skills]]` and still parse — add a `to_toml`→`from_toml` test
  (the existing `[[skills]]` + `[lifecycle]` ordering proves the pattern works).

## Files to change

- **`crates/aenv-core/src/manifest.rs`** — `VendoredDecl` + field + parse + validate.
- **`crates/aenv-core/src/vendor.rs`** (new) — `vendor_into_namespace(...)`: fetch,
  walk, copy, infer adapter, append `files`, append `[[vendored]]`, write once.
- **`crates/aenv-core/src/adapter.rs`** or a small util — shared `adapter_for_path`
  helper (refactor the snapshot inference to call it too).
- **`crates/aenv-cli/src/main.rs`** — `Command::Vendor { source, ns, pin, path,
  as_ /* --as */, force }` clap + dispatch.
- **`crates/aenv-cli/src/cmd/vendor.rs`** (new) — thin CLI handler.
- **`crates/aenv-core/tests/manifest.rs`** — `[[vendored]]` round-trip + validation.
- **`crates/aenv-cli/tests/vendor_e2e.rs`** (new) — tests below.
- **`docs/walkthroughs/install-a-skill-from-github.md`** — replace the "Path B"
  manual clone+cp+edit block with `aenv vendor`.

## Implementation steps (core `vendor_into_namespace`)

1. **Resolve source once** at the pin into the cache (git) or stat the local path.
   `--path` must exist at the ref → else error **before any namespace write**.
2. **Determine destination + adapter.** `--as` gives the namespace-relative dest.
   Infer the adapter from the `--as` prefix via `adapter_for_path`; if ambiguous
   and not declared, error with a hint.
3. **Walk + copy.** `--path file.md` copies one file → `<ns>/<as>`; `--path dir`
   walks recursively → `<ns>/<as>/<rel>`. Resolve symlinks to content (don't copy
   symlinks). A `dest` that already exists → **error unless `--force`**.
4. **Append `files`.** Add each expanded literal dest to the inferred adapter's
   `files` (sorted, deduped). Create the adapter entry if absent.
5. **Record provenance.** Upsert a `VendoredDecl` keyed by `(source, src_path,
   dest)`: on re-run, refresh the copy and **report the per-file change list**
   (idempotent / drift detection); else append a new entry.
6. **Write manifest once.** `to_toml` → `manifest_path`.
7. **Report.** `Vendored N files from <source>@<ref>:<path> into <ns>:<as>.` +
   per-file paths (and, on re-run, the changed/unchanged split).

## Edge cases (all from the issue)

- `--path` absent at `<ref>` → error before any write.
- source file is a symlink → vendor the target's bytes.
- `--as` collides with existing namespace file → error; `--force` overwrites.
- re-run same `(source, path, as)` → idempotent refresh + drift report.
- adapter not inferable from `--as` and not declared → error with hint.
- registry: / local-path sources → handled by the existing resolver dispatch.

## Test plan

- `crates/aenv-core/tests/manifest.rs`: `[[vendored]]` parse + `to_toml`→`from_toml`
  round-trip; `dest` relative-path validation rejects `..`/absolute.
- `crates/aenv-cli/tests/vendor_e2e.rs` (local-fixture, offline):
  - `agents/{a.md,b.md}` + `references/{r.md}`; `vendor --path agents --as
    .claude/agents` copies both, manifest `files` declares both + a `[[vendored]]`
    entry, then project activation symlinks them.
  - single-file vendor `--path agents/a.md --as .claude/agents/a.md`.
  - idempotent re-run → "unchanged", no manifest diff.
  - bump source content + re-vendor → drift reported, per-file change list.
  - `--as` collision without `--force` → errors before any write.
  - Live (network, gated): addyosmani `agents/` + `references/` at `--pin 0.6.1`.

## Verification

- CI parity gate (fmt · clippy `-D warnings` · test · `cargo doc`).
- Real-binary E2E: vendor a local fixture, confirm `files` + `[[vendored]]` +
  activation symlinks; reproduce the rewritten walkthrough "Path B" literally.

## Effort

~1 day. New code: walk-and-copy, the `[[vendored]]` schema + serde + validation,
adapter inference refactor, and the conflict/idempotency checks. Activation path
unchanged.

## Sequencing

- Sibling to #1 (`skill import-all`). Independent; either can land first.
- Prerequisite for #3 (`aenv plugin import`), which dispatches to #1 for `skills/`
  and #2 for `agents/`/`commands/`/`references/`. #3 is out of scope here.
