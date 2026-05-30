# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-05-30

Global-tooling UX simplification: standing up and switching a global profile is now a one-command experience, with safer defaults and a smaller flag surface. (First tagged release since v0.0.3; supersedes the un-tagged v0.1.0 prep below, whose global-namespaces work this release also includes.)

### Added

- **`aenv global use <target>`** — the one-command front door for global profiles. `<target>` is a git URL or local path (imported on the spot if not already present, then activated), an existing namespace name (switch to it), or `-` (toggle back to the previously-active profile). Collapses the former `snapshot` → `import` → `activate` ritual into a single command: `aenv global use https://github.com/juanandresgs/claude-ctrl`. Flags: `--as <name>` (name an imported source), `--pin <ref>` (git sources), `--yes`, `--no-baseline`.
- **`aenv global new <name> [--adapter <a>]`** — scaffold a new, editable user-scope namespace from scratch. Seeds the adapter's instructions file (e.g. `~/.claude/CLAUDE.md`) under the namespace's `user/` subtree and pre-wires the manifest's `user_files`. The third way to create a namespace, alongside `snapshot` and `import`.
- **Auto-baseline on first activation** — the first-ever global activation captures the current `$HOME` user-scope surface into a `baseline` namespace, so there's always a named return point (`aenv global use baseline` or `aenv global use -`). Opt out with `--no-baseline`. An empty `$HOME` captures nothing and leaves no namespace behind.
- **`aenv global doctor --fix`** — deletes the orphan stash directories the audit finds, then reports clean (exit 0).

### Changed

- **`--yes` now covers pre-flight** — `aenv global activate`/`use --yes` proceeds past pre-flight findings without prompting (the scan still runs and prints what it found). The separate `--skip-preflight` flag is gone.
- **Orphan-stash cleanup moved to `aenv global doctor --fix`** — `aenv global deactivate` no longer takes `--prune`; it does exactly one thing.
- **Heuristic import no longer auto-wires `install.sh` / `uninstall.sh` as lifecycle hooks.** A repo's installer is typically a self-installer that wants to own `~/.claude` and fights aenv's materialization (e.g. claude-ctrl's `install.sh` fails: "missing settings.json"). The heuristic now imports config files only; lifecycle hooks are opt-in via `aenv-namespace.toml`. This makes `aenv global use <repo-url>` activate cleanly for config-bearing repos.

### Fixed

- **Imported lifecycle scripts are made executable before running.** The import/snapshot copy path writes bytes only, dropping the source's executable bit; the activator now restores owner-execute before exec'ing an `on_activate` script, so an opt-in hook from a git/path import isn't refused with "Permission denied."

### Deprecated

- **`aenv global activate <ns>`** — use `aenv global use <ns>` instead. The alias still works but prints a deprecation notice. Project-scope `aenv activate` is unaffected.

### Removed

- **`aenv global activate --skip-preflight`** — folded into `--yes` (see Changed).
- **`aenv global deactivate --prune`** — replaced by `aenv global doctor --fix` (see Changed).

## [0.1.0] — 2026-05-28

Issue #4: global namespaces. `aenv` can now swap user-level harness configurations (`~/.claude/`, `~/.codex/`) the same way it swaps project-local configs, with lifecycle hooks for runtime install/uninstall, byte-level recovery, and a separate emergency-recovery binary for hook-lockout scenarios.

### Added

- **`aenv global` subcommand tree** — `snapshot`, `import`, `activate`, `deactivate`, `status`, `which`, `list`, `doctor`, `diff`. Mirrors the project-local verbs but operates on `$HOME` rather than the project root. One global activation per user; activating a new namespace deactivates the prior one in a single transaction. State at `$AENV_HOME/global-state.json`; per-activation stash at `$AENV_HOME/global-stash/<ts>/`; lock at `$AENV_HOME/global.lock` (5-minute stale-PID auto-clear).
- **`aenv global snapshot <name> [--include <path>...]`** — captures every adapter-managed path that exists under `$HOME` (plus `--include` extras) into a new namespace. The starting point for any swap workflow.
- **`aenv global import <source> [<name>] [--pin <ref>]`** — turns a local directory or git URL into a namespace in one command. The source root may ship an `aenv-namespace.toml` (see `pm_docs/aenv-namespace-toml-spec.md`) for authoritative layout mapping; otherwise a built-in heuristic recognizes claude-ctrl-style layouts (`CLAUDE.md`, `agents/`, `commands/`, `hooks/`, `skills/`, `runtime/`, `bin/`, `sidecars/`, `.codex/`, `install.sh`, `uninstall.sh`).
- **`[lifecycle]` manifest section** — namespaces may declare `on_activate` / `on_deactivate` scripts. `on_activate` runs after files are materialized; failure rolls back materialization via the existing undo log. `on_deactivate` runs best-effort before file restoration; failure logs a warning. Scripts receive `AENV_NAMESPACE`, `AENV_SCOPE`, `AENV_TARGET_ROOT`, `AENV_NAMESPACE_DIR`, `AENV_LIFECYCLE_EVENT`, `AENV_FORCE`. Full contract: `pm_docs/lifecycle-hooks.md`.
- **Namespace-scoped, SHA-pinned lifecycle approval** — first activation of a namespace with `on_activate` prompts the user, showing the script path, sha256, and first eight lines. Approval is recorded at `$AENV_HOME/envs/<ns>/.approved` keyed by the script's sha256; editing the script invalidates the approval and re-prompts. `--yes` on `aenv global activate` (and on `aenv use --global`) skips the prompt and records approval.
- **`aenv global deactivate --force`** — skips `on_deactivate` for the case where the lifecycle script itself is broken. File restoration proceeds either way.
- **`aenv-rescue` standalone Rust binary** — emergency deactivate when the active namespace's hooks have locked the user out of their shell. Reads `$AENV_HOME/global-state.json` directly, undoes the activation via direct filesystem operations, never spawns subprocesses (so user hooks don't fire), never invokes `on_deactivate`. Statically linked, no external dependencies (no `jq`).
- **Pre-flight scanner** — `aenv global doctor` (and `aenv global activate`, opt-out via `--skip-preflight`) scans every materialized `settings.json` for hook / MCP server / statusLine command paths that don't exist on disk. Catches the "namespace references a runtime that isn't shipped or installed" lockout class before activation succeeds.
- **`materialize = "copy"`** — per-adapter (`[adapters.<name>] materialize = "copy"`) and per-namespace override (`[adapters.<name>] materialize = "copy"` inside the namespace manifest). Replaces the previous Phase-7-deferred placeholder. Copy-mode materialization writes a regular file copy instead of a symlink; on the next activation the local file is overwritten. `aenv global doctor` emits a `copy_mode_local_edits` warning when a copy-mode target has been edited on disk since activation.
- **`aenv use <name> --global [--yes]`** — sugar that pins the project, activates project-side files, AND activates user-scope files in one invocation.
- **Content hash in `aenv global which --json`** — emits `"content_hash": "sha256:<hex>"` for the matched file's resolved bytes. Enables per-file drift detection by external tooling (the harness-eval consumer named by PRD §5.16).
- **`aenv global deactivate --prune`** — removes orphan stash directories under `$AENV_HOME/global-stash/` after deactivation.
- **`aenv-rescue` binary** ships alongside `aenv` in releases.
- **Exit code 19** — `GlobalConflict`: concurrent global activation, orphan stash with no recorded state, lifecycle hook failure, broken activation rolled back.
- **`pm_docs/lifecycle-hooks.md`** — authoritative lifecycle hook contract (timing, environment, exit codes, REQUIRED author invariants, rollback semantics).
- **`pm_docs/aenv-namespace-toml-spec.md`** — convention file format spec for `aenv global import`.
- **`pm_docs/walkthrough-global-namespaces.md`** — claude-ctrl end-to-end walkthrough (snapshot → import → swap → doctor → recovery).

### Changed

- **`aenv global use <ns>` renamed to `aenv global activate <ns>`** — the new verb name matches the project-side `aenv activate` (which materializes); `aenv use` is project-side pinning, which has no global analog. No backward-compat alias.
- **`aenv use <ns> --global` now also runs `aenv activate` on the project** — earlier this sugar only pinned + globally activated, leaving the project half-materialized. Now it does pin + project-activate + global-activate.
- **`aenv global diff` drift mode is byte-level** (was path-set only).
- **`aenv global list` help text** clarifies the filter — it lists only namespaces declaring user-scope files; `aenv list` shows every namespace.
- **`aenv deactivate --prune`** — project-side analog of the global flag; removes `<project>/.aenv-state/backup/<ts>/` directories after deactivation.
- **`activate_namespace` / `deactivate_namespace` are wrappers** over scope-aware `activate_namespace_in_scope` / `deactivate_namespace_in_scope` (public API unchanged for project-scope callers).
- **`SCHEMA_VERSION` 4 → 6.** Added `scope`, `lifecycle_ran`, `was_present_before_activation`. Old state files load transparently with sensible defaults. The version bump is one-way: v6 readers accept v1-v6 files; v4 readers will not accept v6.
- **`Adapter` struct gains `user_files`, `user_roles`, `user_default_merge`, `user_merge_strategies`, `user_soft_limits`, `user_skills_dir`, `materialize`** — all optional, no breaks.
- **Namespace manifest `[adapters.<name>]` gains `user_files`, `user_merge`, `materialize`** — all optional.
- **`SkillDecl` gains `scope: Scope`** (default `Project`).
- **`Candidate` gains `scope`, `adapter_materialize_override`**.
- **Builtin claude-code / codex / cursor adapters** declare canonical `user_files` (so out-of-the-box snapshots capture the standard surfaces). The claude-code adapter covers `CLAUDE.md`, `agents/`, `commands/`, `hooks/`, `plugins/`, `policy-limits.json`, `settings.json`, `statusline/`, plus skills via `user_skills_dir`. Runtime/state directories (`backups/`, `cache/`, `projects/`, `sessions/`, `history.jsonl`, `.credentials.json`, etc.) are deliberately excluded.
- **Resolver trims trailing `/` on file/`user_files` entries** before constructing target paths — `symlink()` ENOENT regression fix for directory declarations.
- **`aenv global doctor`** reports findings from three synthetic policies: `instructions_max_chars` (existing), `hook_paths_resolvable` (Task 17), `copy_mode_local_edits` (Task 22). Plus orphan-stash detection (Task 20).

### Documentation

- **README "Global namespaces" section** rewritten with honest framing — what aenv does, what the user owns, what's out of scope.
- **`pm_docs/walkthrough-global-namespaces.md`** rewritten to use claude-ctrl as the worked example, including lifecycle approval and recovery.
- **`RELEASING.md`** documents the pre-tag ritual: run the `#[ignore]`-gated `lifecycle_claude_ctrl_real` integration test before tagging.

### Issue

Closes #4.

## [0.0.3] — 2026-05-26

### Added

- **`skills/aenv/SKILL.md`** — a Claude Code skill (importable via `aenv skill import git+https://github.com/Blevene/aenv --ns <ns> --path skills/aenv --pin v0.0.3`) that gives an agent the user-request-to-command map, gotchas, and "when to escalate" rules for aenv operations.
- **Community files** — `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1 by reference), `SECURITY.md` (supported versions + scoped threat model + private-disclosure email).
- **README badges** — CI status, latest release, MIT license, Rust MSRV.

### Changed

- **`aenv --help` descriptions for `use` / `activate` / `deactivate` / `restore`** expanded from one-liners to explain how the commands fit together. `aenv use` now says "Does NOT materialize any files — follow with `aenv activate`"; `aenv activate` names the input (`.aenv` pin or explicit name) and the backup destination; `aenv deactivate` describes the restore-byte-for-byte guarantee; `aenv restore` is framed as the recovery path with copy-vs-move semantics.
- **README reorganized**: Installation / Quick start / Try-the-built-ins / safety / authoring / updating / shell / skills now appear *before* the feature-list and roadmap reference sections, so a new reader's path to first use is shorter.
- **Status line in README** refreshed to reference v0.0.2/Phase 5 (was stuck on `phase-3-complete`).

## [0.0.2] — 2026-05-25

### Changed

- **Linux x86_64 release binary built on `ubuntu-22.04` instead of `ubuntu-latest`.** Lowers the glibc requirement from 2.39 (Ubuntu 24.04) to 2.35 (Ubuntu 22.04), so the released binary now runs on any distro with glibc ≥ 2.35 — Debian 12, RHEL 9, Amazon Linux 2023, etc. v0.0.1's binary failed with `version GLIBC_2.39 not found` on older systems. aarch64-linux was unaffected (built via `cross-rs` in a Docker image with an older glibc baseline).

## [0.0.1] — 2026-05-25

Initial tagged release. Everything described in the README's "What works today" section ships.

### Added

- **Namespace lifecycle** — `aenv create`, `aenv list`, `aenv delete`, `aenv use` (pin), `aenv activate` / `aenv deactivate`, `aenv unpin`, `aenv status`, `aenv which`.
- **Composition** — `extends` chains, section-merged Markdown, deep-merged JSON / YAML / TOML, cycle detection (exit 15), qualified-name provenance on every artifact.
- **Typed parameters and policies** — `[parameters]` (string / int / bool / list-of-string) and `[policies]` blocks inherit across `extends`; `aenv get` / `aenv set` read/write with provenance; R-75 enforce-protection (a child can tighten but not weaken a parent's enforced policy).
- **`aenv doctor`** — four built-in policy evaluators (`instructions_max_chars`, `skill_requires_description`, `mcp_requires_command_or_url`, `forbid_paths`). Enforced violations block `aenv activate` with exit 17 — before any file is touched.
- **Skills lifecycle** — `aenv skill new` (authored, files in the namespace tree), `aenv skill import` (local path or git URL, with `--pin <ref>` and `--path <subdir>` for monorepo skill collections), `aenv skill remove`, `aenv skill list`, `aenv cache prune` to reclaim orphaned `~/.aenv/cache/skills/` directories.
- **`aenv snapshot`** — walks a hand-shaped project against every installed adapter's `files = [...]` and captures the matches into a new namespace; one-way, doesn't update the source project's pin.
- **`aenv diff`** — `aenv diff` for project-vs-namespace drift; `aenv diff <ns_a> <ns_b>` for structural diff between namespaces; both ship `--json`.
- **`aenv fork`** — detach a managed file, a whole project, or fork a namespace into a private copy.
- **Shell integration** — `aenv init-shell <bash|zsh|fish>` emits a hook script; `aenv activate-if-needed` is the fast-path the hook calls on every `chpwd`, transitioning namespaces only when needed. Sub-millisecond no-change path.
- **Scriptability** — `--json` on every read-oriented command (`list`, `status`, `which`, `get`, `doctor`, `skill list`, `adapter list`, `diff`); each namespace also carries a resolved-namespace hash exposed via `aenv status` / `aenv list --json`.
- **Built-in adapters** (8) — claude-code, cursor, aider, cline, continue, windsurf, codex, mcp. Embedded in the binary, written to `~/.aenv/adapters/` on first run, overridable by user edit.
- **Built-in starter namespaces** (2) — `karpathy` (surgical, minimum-code engineering) and `cherny` (plan-first, subagent-heavy). Written to `~/.aenv/envs/` on first run.
- **Safety guarantees** — `aenv activate` backs up displaced files to `.aenv-state/backup/<timestamp>/`; `aenv deactivate` restores byte-for-byte. `aenv restore` is the recovery path when deactivate didn't run cleanly.
- **Release pipeline** — tag-triggered GitHub Actions matrix builds Linux x86_64 / aarch64 + macOS x86_64 / aarch64 binaries, generates checksums, publishes to a GitHub Release. Documented in `RELEASING.md`.

### Not yet shipped

- **Phase 6 (partial)** — `aenv install` / `aenv sync` / `aenv promote` for git-remote-backed multi-machine sync.
- **Phase 7** — Windows symlink fallback to copy-mode + Windows CI; macOS notarization.

[Unreleased]: https://github.com/Blevene/aenv/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Blevene/aenv/compare/v0.0.3...v0.2.0
[0.0.3]: https://github.com/Blevene/aenv/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/Blevene/aenv/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/Blevene/aenv/releases/tag/v0.0.1
