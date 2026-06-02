# Plan: `aenv skill import-all` — bulk skill import from a monorepo (issue #1)

## Context

`aenv skill import` handles one skill per invocation. Monorepo skill collections
(addyosmani/agent-skills = 23 skills under `skills/<name>/SKILL.md`,
k-dense-ai/scientific-agent-skills, …) force the user to repeat the same command N
times with a different `--path` each. This adds a sibling verb that clones the
source **once**, discovers every `<subdir>/SKILL.md` under a base dir, and appends
one `[[skills]]` entry per skill in a single manifest write.

No resolver/activation/schema changes — each discovered skill becomes an ordinary
`[[skills]]` `mode = "imported"` entry that the existing per-skill resolver
materializes exactly as a hand-typed import would.

## Decision (recommended, from the issue)

- **Verb spelling: `aenv skill import-all`** (a sibling subcommand), not
  `skill import --all`. Avoids overloading the existing `import` positional/flags
  and keeps the discovery semantics in their own lane. Confirm before building.

## Reuse inventory (call, don't reimplement)

- `aenv_core::skills::source::SourceKind::parse` — git / local / registry dispatch.
- `aenv_core::skills::git::git_clone(url, ref_spec, dest, sub_path) -> Result<String>`
  — shallow clone (sparse-checkout of `sub_path`), returns the resolved SHA.
- `aenv_core::skills::cache::{skill_cache_path, source_hash}` — cache dir layout
  `~/.aenv/cache/skills/<source-hash>/<ref>/`.
- `aenv_core::skills::mod::{resolve_imported_skill, apply_required_rule}` — used by
  the single-import for pin resolution + frontmatter validation; reuse its
  SKILL.md frontmatter parser (the function `resolve_git`/`resolve_local` already
  parse + validate `name:`).
- `SkillDecl` (`skills/mod.rs`) — the manifest entry struct.
- `AenvManifest::{from_toml,to_toml}` + `RegistryLayout::manifest_path`.
- `walkdir::WalkDir` (already a dependency of aenv-cli) for the discovery walk.
- Existing single-import handler `crates/aenv-cli/src/cmd/skill/import.rs`
  (`skill_name_from_path`, the SkillDecl construction + write sequence) as the
  pattern to mirror.

## Files to change

- **`crates/aenv-cli/src/main.rs`** — add `SkillAction::ImportAll { source, ns,
  base: Option<String>, only: Option<String>, pin: Option<String>, adapter:
  Option<String> }` to the clap enum + a dispatch arm.
- **`crates/aenv-cli/src/cmd/skill/import_all.rs`** (new) — the handler.
- **`crates/aenv-cli/src/cmd/skill/mod.rs`** — register the module.
- **`crates/aenv-cli/tests/skill_import_all_e2e.rs`** (new) — tests below.
- **`docs/walkthroughs/install-a-skill-from-github.md`** — show the bulk form as
  the default for monorepo cases (literally reproduce against the binary).

## Implementation steps (handler)

1. **Resolve source once.** Parse with `SourceKind::parse`. For git: resolve the
   pin to a SHA and `git_clone` into `skill_cache_path(layout, source, <sha-or-"head">)`
   with `sub_path = Some(base)` so only `<base>/` is fetched. For local: stat the
   path directly (no clone). One network round-trip total.
2. **Discover.** Walk `<root>/<base>/` (default `base = "skills"`; if omitted, also
   accept SKILL.md directly at the cache root for single-skill repos). Each
   immediate `<subdir>` containing a `SKILL.md` is a candidate; `name =
   <subdir>` basename, `path = "<base>/<subdir>"`.
3. **`--only` filter.** Parse the comma list into a set; keep only matching names.
   **Any name in `--only` not found among candidates → error before any manifest
   write**, listing the missing names.
4. **Validate frontmatter.** For each candidate, parse `SKILL.md` YAML frontmatter
   (reuse the skills resolver's parser); a missing/!`name:` frontmatter → **warn,
   naming the subdir, and skip that one** (don't fail the whole run).
5. **Dedup vs manifest.** Skip candidates whose `name` already exists in
   `manifest.skills` (idempotent re-run); collect them for the report.
6. **Batch append + single write.** Build a `SkillDecl` per surviving candidate
   (`mode: Imported`, `adapter` (explicit or inferred when the namespace has one
   adapter), `source`, `ref_ = <resolved SHA>`, `path`, `required: false`, `scope:
   Project`); push all; `fs.write(manifest_path, manifest.to_toml())` **once**.
7. **Report.** One line per imported skill + a final
   `Imported N skills from <source> @ <ref> into namespace <ns>.`; separately list
   skipped (already-declared) and warned (malformed) skills.

## Edge cases (all from the issue)

- `--base` resolves to a dir with no `<subdir>/SKILL.md` → error with the hint
  ("no `<subdir>/SKILL.md` under '<base>' — check the path or omit `--base`").
- empty / malformed `SKILL.md` → warn + skip, continue.
- name already declared → skip (idempotent), note in output.
- `--only foo,bar` with `bar` absent → error before any write, name the missing.
- local-path monorepo source → walk the local `<base>` directly, `ref_` omitted.

## Test plan

`crates/aenv-cli/tests/skill_import_all_e2e.rs` (local-fixture monorepo, offline):
- 3 subdirs (2 valid SKILL.md, 1 malformed) → imports 2, warns about 1, exit 0;
  manifest has exactly 2 new `[[skills]]`.
- `--only` happy path filters to the named subset.
- `--only` unknown name → errors, manifest unchanged.
- idempotent re-run → second invocation reports "already declared", no double-write.
- Live (network, mirror the existing `skill_import_git_e2e` gating): bulk import
  `git+https://github.com/addyosmani/agent-skills --base skills --pin 0.6.1` →
  23 entries with the expected resolved SHA, then `aenv activate` materializes all.

## Verification

- CI parity gate: `fmt` · `clippy --all-targets -D warnings` · `cargo test
  --workspace` · `RUSTDOCFLAGS="-D warnings" cargo doc`.
- Real-binary E2E: run the bulk import against a local fixture and (if network)
  the addyosmani repo; confirm 23 `[[skills]]` + activation materializes them.
- Reproduce the updated walkthrough section literally.

## Effort

~Half a day. New code is the discovery walk + `--only` filter + batch append; all
fetch/cache/resolution/validation is reused.

## Out of scope / sequencing

- `aenv vendor` (issue #2) — sibling, planned separately.
- `aenv plugin import` (#3) — depends on both #1 and #2; not in scope here.
