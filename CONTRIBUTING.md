# Contributing to aenv

Thanks for the interest. A few practical notes before you open a PR.

## Setup

- Rust toolchain **1.85 or newer** (MSRV). Install via [rustup](https://rustup.rs).
- A POSIX-y filesystem with symlink support (Linux or macOS — Windows is Phase 7).

```bash
git clone https://github.com/Blevene/aenv
cd aenv
cargo build --workspace
```

## What every PR has to pass locally

```bash
cargo test --workspace                            # all suites, ~500 tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

CI runs the same checks across Linux and macOS plus a Windows compile-only canary (`x86_64-pc-windows-gnu`). The MSRV job runs `cargo +1.85.0 check --workspace --all-targets --locked` — please don't bump the MSRV without flagging it in the PR description.

## What a good PR looks like

- **Focused.** One logical change per PR. Easier to review, easier to revert.
- **Tested.** New behavior gets a test. Bug fixes get a regression test. The repo aims for "the test would have caught this" — if you can't write that test, mention why.
- **Documented.** If your change touches user-facing behavior (a new flag, a new command, a new file aenv writes), the README and any relevant walkthrough in `docs/walkthroughs/` need to match. Walkthroughs have been validated end-to-end against the live binary — please re-verify yours.
- **Honest commit messages.** Subject in the imperative present tense ("Skills: add skill remove", not "added skill remove"). Body explains the *why* and any non-obvious tradeoffs. Existing history is a reasonable reference.

## Don't

- Skip CI hooks (`--no-verify`, `--no-gpg-sign`) without saying so explicitly in the PR.
- Add backwards-compatibility shims or "for future use" abstractions unless the PR cites a concrete near-term need.
- Bump the MSRV silently.
- Commit `target/`, `.aenv/`, `.aenv-state/`, or anything matching `~/.cargo/*`.
- Open a PR that doesn't run cleanly through `cargo test && cargo clippy -- -D warnings && cargo fmt --check`.

## Repo layout

- `crates/aenv-core/` — library: namespace model, resolution, materialization, skills, parameters, policies. No I/O via env vars or `current_dir()`; the CLI layer resolves those.
- `crates/aenv-cli/` — binary + CLI handlers. Owns env-var / cwd resolution + user-facing output.
- `pm_docs/` — design rationale: PRD (87 requirements, R-1 through R-87), functional spec (user journeys), engineering notes.
- `tasks/` — phase-by-phase implementation plans (historical artifacts after each phase completes).
- `docs/walkthroughs/` — user-facing recipes, validated against the live binary.
- `.github/workflows/` — CI (`ci.yml`) and release (`release.yml`).

## Reporting bugs

Open a GitHub issue with:
- What you ran
- What you expected
- What you got
- Versions: `aenv --version`, `rustc --version`, OS + version

For security issues, see [SECURITY.md](./SECURITY.md) — don't open a public issue.

## Releases

Maintainer-only flow lives in [RELEASING.md](./RELEASING.md). The TL;DR: bump `workspace.package.version`, push a `v*` tag, the release workflow builds the four-target matrix and publishes the GitHub Release.

## Code of conduct

Participation in this project is governed by the [Code of Conduct](./CODE_OF_CONDUCT.md).
