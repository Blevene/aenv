# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/Blevene/aenv/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/Blevene/aenv/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/Blevene/aenv/releases/tag/v0.0.1
