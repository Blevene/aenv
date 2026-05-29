# Global Tooling UX Simplification

> **For agentic workers:** TDD per change. `cargo` is not on PATH — prefix every invocation with `PATH="$HOME/.cargo/bin:$PATH"`. Style: `rustfmt max_width = 100`, `clippy -D warnings`. Commit per change directly on `main`; don't push.

**Goal:** The global namespaces feature shipped as v0.1.0, but standing up and switching a global profile is a multi-command ritual. This pass makes it a **one-command experience** and trims incidental flag/verb friction — without removing capability. `aenv-rescue` stays (it's a safety net, not friction).

**No state schema bump** for the `ActivationState` struct. `SCHEMA_VERSION` stays 6. (One tiny new sidecar file `global-previous` — name only — is added for the `use -` toggle; not part of the schema'd state.)

**The pain we're killing.** Onboarding a profile today is:
```
aenv global snapshot default            # remember a return point
aenv global import <url> <name>         # bring it in
aenv global activate <name>             # turn it on
```
After this pass it's:
```
aenv global use <url>                   # baseline + import + activate, one shot
```
and swapping back is `aenv global use baseline` (or `aenv global use -`).

---

## Change 5 (HEADLINE) — `aenv global use <target>`: one-command onboarding + swap

**Target:** A single front-door verb that does the right thing based on `<target>`:

- **git URL / local source dir** → import it (name derived from the source, or `--as <name>`), then activate.
- **existing namespace name** → switch the active global profile to it.
- **`-`** → switch to the previously-active profile (toggle).

Auto-baseline (Change 2) fires on the first-ever activation regardless of which path above ran. `activate` is retained as a thin **alias** for the name/`-` cases so v0.1.0 muscle memory and scripts keep working.

**Target resolution precedence (deterministic):**
1. `target == "-"` → previous profile (read `<aenv_home>/global-previous`).
2. URL-shaped (`https://`, `http://`, `git://`, `git@`, `file://`, `*.git`) → import-then-activate.
3. A namespace with that exact name exists → activate it (name wins over a coincidental same-named local path).
4. A local directory exists at that path → import-then-activate.
5. else → `NamespaceNotFound`.

**Flags:** `--as <name>` (override derived import name; ignored for name/`-` targets), `--yes` (Change 1 semantics), `--no-baseline` (Change 2 opt-out), `--pin <ref>` (git source pin, mirrors `import`).

**Previous-profile tracking:** whenever a swap deactivates an outgoing namespace, write its name to `<aenv_home>/global-previous`. `use -` reads it; errors cleanly if absent.

**Files:**
- `crates/aenv-cli/src/main.rs` — add `GlobalAction::Use { target, as_name, yes, no_baseline, pin }`; keep `Activate` as an alias variant (clap `#[command(alias = "activate")]` or a separate arm that calls the same `run`). Dispatch resolves precedence, calling `import_global` (for source) and the shared activate path.
- `crates/aenv-cli/src/cmd/global/use_.rs` (new) — orchestration: resolve target, optional import, auto-baseline, activate, record `global-previous`. Reuse `cmd::global::import`'s URL detection + name derivation (extract a shared helper if needed).
- `crates/aenv-core/src/home.rs` — add `global_previous_path()`.
- `crates/aenv-cli/tests/global_use_e2e.rs` (new) — tests: (a) `use <local-source-dir>` imports + activates in one shot; (b) `use <existing-name>` swaps; (c) `use -` toggles to prior; (d) `--as` overrides import name; (e) name precedence over coincidental path; (f) `activate` alias still works.

**Verify:** new e2e green; `aenv global use https://github.com/juanandresgs/claude-ctrl` onboards in one command (covered by the gated real-network test, repointed).

---

## Change 6 — `aenv global new <name>`: scaffold an editable user-scope namespace

**Target:** Authoring your own profile from scratch shouldn't mean manual `mkdir user/.claude/` + hand-writing the manifest. `aenv global new <name>` creates:
- `<aenv_home>/envs/<name>/user/.claude/CLAUDE.md` (empty starter file),
- `<aenv_home>/envs/<name>/aenv.toml` with `[adapters.claude-code] files = []` and `user_files = [".claude/CLAUDE.md"]` pre-wired,
then prints the path to edit and the `aenv global use <name>` command to activate it.

`--adapter <a>` selects the adapter (default `claude-code`); the scaffold uses that adapter's `user_skills_dir` / conventional paths.

**Files:**
- `crates/aenv-cli/src/main.rs` — add `GlobalAction::New { name, adapter }`.
- `crates/aenv-cli/src/cmd/global/new.rs` (new) — scaffold logic; refuse if the namespace exists (reuse the freshness check from `snapshot_global`/`import_global`).
- `crates/aenv-cli/tests/global_new_e2e.rs` (new) — tests: scaffolds the tree + manifest; the result is immediately `use`-able; refuses an existing name.

**Verify:** `aenv global new mine && aenv global use mine` round-trips; manifest parses.

---

## Change 1 — Collapse `--yes` and `--skip-preflight` into `--yes`

**Today:** `activate` takes `--yes` (approve lifecycle + auto-answer preflight "yes") *and* `--skip-preflight` (suppress the scan). Two near-duplicate opt-outs.

**Target:** Single `--yes` = "non-interactive: I trust this namespace." Under `--yes` the preflight scan still runs and prints findings but doesn't prompt; lifecycle approval is recorded without prompting. `--skip-preflight` removed. Applies to both `use` and the `activate` alias. **Lost (niche, documented):** "prompt for lifecycle but silence the scan."

**Files:**
- `crates/aenv-cli/src/main.rs` — drop `skip_preflight` from the activate/use surface and dispatch; `--global` sugar (≈451–477) passes `yes=true` only.
- `crates/aenv-cli/src/cmd/global/activate.rs` — drop `skip_preflight` param (22–31); preflight block (38–76) always scans, prompts only when `!yes`.
- `crates/aenv-cli/tests/global_activate_e2e.rs` — drop `--skip-preflight` usages; add: `--yes` proceeds past preflight findings without prompting.

---

## Change 2 — Auto-baseline on first activate (safer default)

**Target:** On the **first-ever** global activation (no live `global-state.json` **and** no `baseline` namespace **and** `!--no-baseline`), auto-snapshot the current `$HOME` user-scope surface into a namespace named **`baseline`**, then proceed. If nothing was captured (empty surface), delete the empty namespace and stay silent. On capture, print: `Captured your current ~/ surface as 'baseline' (swap back with: aenv global use baseline).`

> Naming: `baseline` (clearer than `default`, and leaves `default` free for the user). Trivial to change — flag if you'd prefer `default`.

**Files:**
- `crates/aenv-cli/src/cmd/global/use_.rs` / `activate.rs` — run the guard before the activate core (`swap_or_activate_user`); call `snapshot_global(fs, layout, adapters, fake_home, "baseline", &[])`; if `files_copied + directories_copied == 0`, `remove_dir_all` the namespace dir and skip the note.
- `crates/aenv-cli/src/main.rs` — `--no-baseline` on the activate/use surface.
- tests (in `global_use_e2e.rs`): (a) first activate with seeded `~/.claude` creates reactivatable `baseline`; (b) empty `$HOME` → no baseline ns; (c) `--no-baseline` suppresses; (d) second activate does not re-snapshot.

---

## Change 4 — Move orphan-stash `--prune` from `deactivate` to `doctor --fix`

**Target:** Pair detection with remediation: `aenv global doctor --fix` prunes orphan stashes (`state::list_orphan_stashes` + `remove_dir_all`) and exits 0; without `--fix`, keeps current exit-19 detection. Remove `--prune` from `deactivate` so it does one thing.

**Scope:** global/user-scope orphan stashes only. Verify whether project-side `deactivate` has its own `--prune` (`deactivate_prune_e2e.rs`); if so leave it untouched.

**Files:**
- `crates/aenv-cli/src/main.rs` — remove `prune` from `GlobalAction::Deactivate` (332–342); add `fix: bool` to `GlobalAction::Doctor`.
- `crates/aenv-cli/src/cmd/global/deactivate.rs` — remove prune block (47–62) + `prune` param (17–23).
- `crates/aenv-cli/src/cmd/global/doctor.rs` — `--fix` prunes orphans, reports count, exit 0.
- `crates/aenv-cli/tests/global_orphan_stash_e2e.rs` — repoint `deactivate --prune` → `doctor --fix`.

---

## ~~Change 3 — Fold aenv-rescue into deactivate --force~~ — DROPPED

User keeps `aenv-rescue` as the corruption-proof safety net. No change to rescue or `--force`.

---

## Docs (all changes)

- `README.md` global section + `pm_docs/walkthrough-global-namespaces.md`: rewrite the onboarding flow around `aenv global use`; document `global new`, `use -`, auto-baseline, the collapsed `--yes`, and `doctor --fix`. Keep the `aenv-rescue` recovery section as-is.
- Repoint the gated real-network test (`lifecycle_claude_ctrl_real.rs`) to the one-command `use <url>` path.

## Sequencing

All touch `main.rs`'s `GlobalAction` enum + dispatch and `cmd/global/*.rs` — implement **sequentially in one session**, one commit per change, each gated on its tests + `clippy -D warnings` + `fmt`.

Order: **1 → 4 → 6 → 2 → 5.** (1 and 4 are small clap edits; 6 adds an independent verb; 2 adds the baseline guard; 5 is the headline that ties source-import + baseline + swap + `-` together and depends on 2 being in place.)

## Final verification
- `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --check` (all `PATH=`-prefixed).
- Walk the README/walkthrough end-to-end for behavior drift.
- Capture user corrections in `tasks/lessons.md`.

---

## Review (executed 2026-05-29)

All five approved changes landed; Change 3 (folding `aenv-rescue`) was **dropped** at the user's direction — rescue stays as the corruption-proof safety net. Commits on `main`:

- `c87b0fd` Change 1 — collapse `--yes`/`--skip-preflight` → `--yes`.
- `9db1255` Change 4 — orphan `--prune` moved from `deactivate` to `doctor --fix` (surfaced + fixed a latent exit-13 bug in the no-activation `--fix` path).
- `fe68522` Change 6 — `aenv global new` scaffolds an editable user-scope namespace.
- `95b3c72` Change 2 — auto-baseline on first activation (`--no-baseline` opt-out).
- `be6ecb0` Change 5 — `aenv global use <target>` front door (URL/path/name/`-`), `activate` kept as alias; gated real-network test repointed.
- `dde253b` Docs — README + walkthrough rewritten around the new UX.
- `70c74f4` Fix — `use -` returns to `baseline` immediately after the first onboard (first activation had no prior profile to record).

**Verification:** 132 test binaries green, `clippy -D warnings` clean, `fmt --check` clean. End-to-end smoke test confirmed one-command onboarding + `use -` toggle on the real binary.

**Net UX win:** onboarding a profile went from 3 commands (`snapshot` + `import` + `activate`) to 1 (`aenv global use <url>`); authoring from scratch got a real `global new` scaffold; swapping is unified under `use` with a `-` toggle and a named `baseline` return point.

**Investigation note:** a smoke test showed `cherny`/`karpathy` namespaces appearing in isolated temp registries. Root cause: these are **intentional built-in example namespaces** (`crates/aenv-core/src/namespaces_builtin/mod.rs`, embedded via `include_str!`) that auto-seed every registry — pre-existing v0.1.0 behavior, not a regression and not from this work. Recorded in auto-memory to avoid re-investigating.
