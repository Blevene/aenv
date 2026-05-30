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
