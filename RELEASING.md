# Releasing aenv

This document is for maintainers cutting a new `aenv` release. The release pipeline is a tag-triggered GitHub Actions workflow (`.github/workflows/release.yml`) that builds four pre-compiled binaries and attaches them to a GitHub Release.

End-user install instructions live in [`INSTALL_FROM_BINARY.md`](./INSTALL_FROM_BINARY.md) (binary download) and the README's [Installation section](./README.md#installation) (build from source).

## How the release pipeline works

| Trigger | `push` of a tag matching `v*` |
| Matrix | 4 targets, see below |
| Output | one `.tar.gz` + `.sha256` per target, attached to a GitHub Release |

### Matrix

| Target triple | Runner | Build path |
|---|---|---|
| `x86_64-unknown-linux-gnu` | `ubuntu-22.04` (pinned for glibc 2.35 portability) | Native `cargo build` |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` | [`cross`](https://github.com/cross-rs/cross) (Docker; older glibc baseline) |
| `x86_64-apple-darwin` | `macos-15-intel` | Native `cargo build` |
| `aarch64-apple-darwin` | `macos-15` | Native `cargo build` |

Windows is intentionally absent until the symlink fallback lands in Phase 7. The existing `ci.yml` `windows-check` job keeps the Windows codepath compiling in the meantime.

### Per-target artifact

```
aenv-<version>-<target>.tar.gz
└── aenv-<version>-<target>/
    ├── aenv          # the binary
    ├── LICENSE
    └── README.md

aenv-<version>-<target>.tar.gz.sha256    # shasum -a 256 of the tarball
```

`<version>` is the tag with the leading `v` stripped (e.g. `v0.0.3` → `0.0.3`).

### Release-creation step

After all four matrix jobs succeed, the `release` job uses the preinstalled `gh` CLI to:

1. Download every matrix artifact.
2. `gh release create <tag> --title "aenv <version>" --generate-notes <tarballs>` — `--generate-notes` pulls the commit log since the previous tag for the release body.

The release is created as **published** (not draft). If you want a draft for manual edits before publishing, pass `--draft` in the workflow's `gh release create` line.

## Cutting a release

### 1. Pre-flight

- All CI is green on `main`.
- `cargo test --workspace` passes locally.
- Working tree is clean.
- You are on `main` and up to date with origin: `git pull --ff-only origin main`.

### 2. Bump the version

Edit `Cargo.toml` at the workspace root:

```toml
[workspace.package]
version = "0.0.2"   # ← bump
```

Regenerate `Cargo.lock` so the lockfile reflects the new workspace version:

```bash
cargo build --workspace
```

Commit:

```bash
git add Cargo.toml Cargo.lock
git commit -m "Release: vX.Y.Z"
git push origin main
```

### 3. Tag and push

```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

Pushing the tag triggers `release.yml`. Track it under the repo's Actions tab. The full pipeline (4 builds in parallel + release job) typically takes 8–12 minutes; the aarch64-linux cross build is usually the long-pole because it spins up a Docker image.

### 4. Verify the release

Once the workflow finishes:

```bash
gh release view vX.Y.Z
```

Should list four `.tar.gz` files and four `.sha256` files. Pull one down and smoke-test:

```bash
VERSION=0.0.2
TARGET=$(uname -sm | awk '
    /Darwin arm64/   {print "aarch64-apple-darwin"}
    /Darwin x86_64/  {print "x86_64-apple-darwin"}
    /Linux x86_64/   {print "x86_64-unknown-linux-gnu"}
    /Linux aarch64/  {print "aarch64-unknown-linux-gnu"}')
curl -LO "https://github.com/blevene/aenv/releases/download/v${VERSION}/aenv-${VERSION}-${TARGET}.tar.gz"
tar -xzf "aenv-${VERSION}-${TARGET}.tar.gz"
"./aenv-${VERSION}-${TARGET}/aenv" --version
```

## Dry-running the pipeline

Before a real release, you can validate the workflow end-to-end against a throwaway tag:

```bash
git tag v0.0.0-rc1
git push origin v0.0.0-rc1
# ... watch the Actions tab, verify all four artifacts attach correctly ...
# Then clean up:
gh release delete v0.0.0-rc1 --yes
git push --delete origin v0.0.0-rc1
git tag -d v0.0.0-rc1
```

Pre-release tags (`vX.Y.Z-anything`) match the `v*` filter, so the workflow fires just as it would for a stable tag.

## Rolling back a release

GitHub Releases can be deleted; binaries cannot be unringed once anyone has downloaded them. Prefer to ship a follow-up fix release rather than yanking, except in security-sensitive cases.

If you must yank:

```bash
gh release delete vX.Y.Z --yes
git push --delete origin vX.Y.Z
git tag -d vX.Y.Z
```

Then publish a fix release at `v0.0.3` (don't reuse the yanked tag — caches and mirrors may still serve it).

## Troubleshooting

**aarch64-linux build fails inside `cross`.** Almost always a Docker / network glitch. Re-run that one job from the Actions tab; cross fetches its container image fresh each run.

**macOS Intel build fails with "runner unavailable".** `macos-15-intel` rotates — if GitHub bumps to `macos-16-intel`, update the matrix. The available labels are listed in [GitHub's runner-images docs](https://github.com/actions/runner-images).

**`gh release create` complains "release already exists".** A tag of the same name has been pushed before. Either delete the existing release (`gh release delete <tag>`) or bump the version.

**Workflow doesn't trigger on tag push.** Ensure you pushed the tag (`git push origin <tag>`), not just the commit — `git push --tags` works too. Tags pushed before the workflow file existed on the default branch will not trigger.
