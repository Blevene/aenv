# Cross-machine hash fixtures

Two small namespaces (`alpha` and `beta`, where `beta extends alpha`)
with hand-computed expected hashes. The accompanying test
`crates/aenv-core/tests/cross_machine_hash.rs` recomputes each hash
and asserts it matches the line in `expected.txt`.

Any change to a fixture file (including whitespace) requires
regenerating the corresponding line in `expected.txt`. To regenerate:

    PATH="$HOME/.cargo/bin:$PATH" cargo test -p aenv-core --test cross_machine_hash \
      -- --nocapture --ignored print_hashes

That prints the current hashes; copy them into `expected.txt`.

The `.gitattributes` file in this directory forces LF on every text
file so the hash stays platform-stable across Linux/macOS/Windows.

Adding a new fixture: create `<name>/aenv.toml` + supporting files,
regenerate `expected.txt`, commit.
