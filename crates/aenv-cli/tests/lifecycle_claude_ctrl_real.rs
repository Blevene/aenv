//! Real claude-ctrl integration. Onboards the upstream repo in one command
//! (`aenv global use <url>` = import + activate) under a tempdir-HOME with
//! `--yes`, asserts the key materialization markers are in place, swaps back
//! to a `default` snapshot, then deactivates and asserts restoration.
//!
//! This test:
//! - clones from a public github repo (network required),
//! - runs claude-ctrl's lifecycle (Python + pip + whatever it needs),
//! - is slow (>30s typical).
//!
//! Gated `#[ignore]` so CI without those deps doesn't fail. Run with:
//!
//! ```bash
//! cargo test -p aenv-cli --test lifecycle_claude_ctrl_real -- --ignored
//! ```
//!
//! before each release (see RELEASING.md "Pre-tag ritual").
//!
//! The assertions check SHAPE markers (specific files exist, exit codes
//! are zero) rather than exact bytes — claude-ctrl evolves and we don't
//! want to pin to a particular commit. If claude-ctrl renames a file we
//! depend on, this test will surface that with a clear failure rather
//! than producing misleading green.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
#[ignore = "real-network + pip + node required; pre-release ritual"]
fn claude_ctrl_imports_activates_deactivates_clean() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();

    // 1. Snapshot the (empty) fake_home as the "default" baseline so
    //    we have something to swap back to at the end of the cycle.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "snapshot", "default"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "snapshot failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // 2. Onboard claude-ctrl from upstream in ONE command: `global use <url>`
    //    imports it as `claude-cntrl` and activates it (with --yes so the
    //    lifecycle approval prompt doesn't block). --no-baseline because we
    //    already captured `default` as our return point above.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args([
            "global",
            "use",
            "https://github.com/juanandresgs/claude-ctrl",
            "--as",
            "claude-cntrl",
            "--yes",
            "--no-baseline",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "use <url> failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // 4. Spot-check materialization markers. SHAPE only — claude-ctrl
    //    evolves, so we don't pin to specific bytes. If a file we
    //    depend on gets renamed upstream, this is the clear failure
    //    site for that.
    assert!(
        fake_home.join(".claude/CLAUDE.md").exists(),
        "CLAUDE.md not materialized under fake_home/.claude/"
    );
    assert!(
        fake_home.join(".claude/settings.json").exists(),
        "settings.json not materialized under fake_home/.claude/"
    );
    // claude-ctrl ships hooks/; verify at least the directory was
    // materialized. (Either symlink or copy — we don't care which.)
    assert!(
        fake_home.join(".claude/hooks").exists(),
        "hooks/ not materialized under fake_home/.claude/"
    );

    // 5. Swap to the empty "default" snapshot via `use <name>`. This is the
    //    activate-while-already-active transaction (deactivate the current
    //    namespace, materialize the new one, all in one shot).
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "use", "default", "--yes", "--no-baseline"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "swap-to-default failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // 6. Final deactivate. The original (empty) fake_home should be
    //    restored.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "deactivate failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Empty fake_home is restored — the empty snapshot had nothing to
    // materialize, so any `.claude/CLAUDE.md` left over here would
    // signal a deactivate leak.
    let claude_md = fake_home.join(".claude/CLAUDE.md");
    if claude_md.exists() {
        let bytes = std::fs::read(&claude_md).unwrap();
        assert!(
            bytes.is_empty(),
            "expected empty/missing CLAUDE.md after deactivate; got {} bytes",
            bytes.len()
        );
    }
}
