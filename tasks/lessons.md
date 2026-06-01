# Lessons

Patterns captured after corrections, to avoid repeating mistakes.

## Local verification must include the doc gate (2026-05-30)

**Mistake:** Pushed a `cargo doc` failure to CI when releasing v0.2.0. My local
checks were `cargo test` + `cargo clippy` + `cargo fmt` only — none of which
catch rustdoc lints. CI's **"Doc build (warnings as errors)"** step
(`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`) failed on a
public-doc-links-private-item warning I introduced, plus pre-existing ones
that had been latent for days (rustdoc stops at the first failing crate, so
doc errors mask each other).

**Rule:** Before pushing aenv changes, run all four CI gates locally:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets -- -D warnings
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace
PATH="$HOME/.cargo/bin:$PATH" RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
```

Common rustdoc-only failures: a public item's doc linking a **private** item
(`rustdoc::private-intra-doc-links`); `[[double-bracket]]` text read as an
unresolved intra-doc link; a bare `<placeholder>` in a doc comment parsed as an
"unclosed HTML tag". Fix by wrapping placeholders in backticks and not linking
private items from public docs. When it fails, fix every crate (re-run to exit
0) — CI only shows one crate's errors at a time.

## Doc commands: verify behavior, not just existence; literal-reproduce walkthroughs (2026-05-31)

**Mistake:** In a docs "accessibility pass" I added recovery sections and a
glossary whose behavioral claims were sourced from `--help` text and inference,
not from running the binary. Two were wrong: I rewrote a snapshot claim by
*inferring* from an internal contradiction (it happened to be right only because
I later ran it), and I quoted `deactivate --help`'s "deletes `.aenv-state/`"
which is overstated (it retains `backup/<ts>/` until `--prune`). When the user
asked "are we consistent — do we need to re-test E2E?", a spot-check immediately
found a *real bug*: bare `aenv snapshot <name>` from cwd failed "project not
pinned" (exit 20) — and the user-facing `docs/walkthroughs/` had never been
literally reproduced, only accessibility-reviewed. Reproduction then caught
output drift in 5 of 6 (file ordering, use-vs-activate output, silent shell hook).

**Rules:**
- A doc claim about *behavior* (output, exit code, what a command does/leaves
  behind) must be verified by *running the command*, never by quoting `--help`
  or inferring. `--help` itself can be wrong — verify it too.
- Before "fixing" a behavioral claim, check for an existing test that encodes
  the contract (`deactivate_prune_e2e` proved the `.aenv-state` retention was
  intentional, not a bug — I'd nearly "fixed" a real behavior).
- Tutorial-style docs (`docs/walkthroughs/`) need the same literal reproduction
  the pm_docs got — accessibility review (prose) will not catch output drift.
- Watch for test blind spots: every snapshot e2e test passed `--project`, so
  none exercised the cwd path the walkthrough actually documents. When a bug
  hides behind a flag all tests happen to pass, add the un-flagged regression.
