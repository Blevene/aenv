# Installing aenv from a pre-built binary

Pre-built binaries for Linux and macOS are published to the [GitHub Releases page](https://github.com/blevene/aenv/releases) on every `v*` tag. Windows is not yet supported (the `aenv activate` codepath needs the symlink fallback landing in Phase 7); Windows users should [build from source](./README.md#installation) or wait for v0.1.0.

## Pick the right tarball

| Your machine | Tarball |
|---|---|
| Linux on Intel/AMD (most desktops, servers, WSL) | `aenv-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux on ARM64 (Raspberry Pi 4+, AWS Graviton, Ampere) | `aenv-<version>-aarch64-unknown-linux-gnu.tar.gz` |
| macOS on Intel | `aenv-<version>-x86_64-apple-darwin.tar.gz` |
| macOS on Apple Silicon (M1 / M2 / M3 / M4) | `aenv-<version>-aarch64-apple-darwin.tar.gz` |

Not sure? Run `uname -sm` — it prints `<OS> <arch>` (e.g. `Darwin arm64` → Apple Silicon, `Linux x86_64` → Intel/AMD Linux).

## Install

The steps are the same on Linux and macOS; substitute the tarball name for your platform.

```bash
# 1. Set version (latest release; check the Releases page)
VERSION=0.2.1
TARGET=aarch64-apple-darwin   # or x86_64-unknown-linux-gnu, etc.

# 2. Download the tarball and its checksum
curl -LO "https://github.com/blevene/aenv/releases/download/v${VERSION}/aenv-${VERSION}-${TARGET}.tar.gz"
curl -LO "https://github.com/blevene/aenv/releases/download/v${VERSION}/aenv-${VERSION}-${TARGET}.tar.gz.sha256"

# 3. Verify the checksum
shasum -a 256 -c "aenv-${VERSION}-${TARGET}.tar.gz.sha256"
# → aenv-<version>-<target>.tar.gz: OK

# 4. Extract
tar -xzf "aenv-${VERSION}-${TARGET}.tar.gz"

# 5. Install into your PATH (~/.local/bin is on most users' PATH;
#    /usr/local/bin works too but needs sudo on most setups)
mkdir -p ~/.local/bin
mv "aenv-${VERSION}-${TARGET}/aenv" ~/.local/bin/

# 6. Clean up
rm -rf "aenv-${VERSION}-${TARGET}" "aenv-${VERSION}-${TARGET}.tar.gz"*
```

If `~/.local/bin` is not on your `PATH`, add it to your shell profile:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc   # or ~/.bashrc
```

### macOS-only: clear the quarantine flag

macOS attaches a quarantine attribute to any file downloaded via a browser or `curl`. Because the binary isn't (yet) notarized, the first `aenv` invocation will be blocked by Gatekeeper with `"aenv" cannot be opened because the developer cannot be verified`. Clear the flag once:

```bash
xattr -d com.apple.quarantine ~/.local/bin/aenv
```

This is a one-time operation per downloaded binary. (Notarization is on the roadmap for v0.1.0.)

## Verify

```bash
aenv --version            # → aenv <version>
aenv list                 # → karpathy and cherny (the starter namespaces)
```

The first invocation populates `~/.aenv/` with the eight built-in adapters and the two starter namespaces — see the [`Installation` section of the README](./README.md#installation) for the full layout.

## Updating

Re-run the install steps with the new `VERSION`. The new binary replaces the old one; your `~/.aenv/` registry is untouched, so any namespaces you've edited or created survive across upgrades.

## Uninstalling

```bash
rm ~/.local/bin/aenv      # remove the binary
rm -rf ~/.aenv             # optional: discard the registry and your namespaces
```
