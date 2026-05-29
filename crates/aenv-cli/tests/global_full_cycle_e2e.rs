//! Full-cycle integration: activate ns1 globally, swap to ns2, deactivate
//! with prune, against a tempdir standing in for the user's $HOME.
//!
//! Exercises the entire user-scope lifecycle end-to-end against the
//! embedded builtin `claude-code` adapter so the test reflects what
//! ships, not a synthetic adapter.

use std::path::{Path, PathBuf};
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

/// Write a namespace with multi-file user-scope content: CLAUDE.md, an
/// agent, and a settings.json fragment.
fn write_ns(aenv_home: &Path, name: &str, claude_body: &[u8], agent_body: &[u8]) {
    let ns_dir = aenv_home.join("envs").join(name);
    std::fs::create_dir_all(ns_dir.join("user/.claude/agents")).unwrap();
    std::fs::create_dir_all(ns_dir.join("user/.claude/commands")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), claude_body).unwrap();
    std::fs::write(ns_dir.join("user/.claude/agents/explorer.md"), agent_body).unwrap();
    std::fs::write(
        ns_dir.join("user/.claude/settings.json"),
        format!(r#"{{"namespace":"{name}"}}"#),
    )
    .unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        format!(
            r#"name = "{name}"
[adapters.claude-code]
user_files = [
    ".claude/CLAUDE.md",
    ".claude/agents/explorer.md",
    ".claude/settings.json",
]
"#
        ),
    )
    .unwrap();
}

#[test]
fn full_cycle_activate_swap_deactivate_prune() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();

    // Pre-existing user file we expect aenv to stash on first activation.
    std::fs::create_dir_all(fake_home.join(".claude/agents")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original CLAUDE.md").unwrap();

    write_ns(
        &aenv_home,
        "research",
        b"# Research mode",
        b"research agent",
    );
    write_ns(&aenv_home, "default", b"# Default mode", b"default agent");

    // 1. global activate research.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "activate", "research"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global activate research failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"# Research mode"
    );
    assert!(fake_home.join(".claude/agents/explorer.md").exists());
    assert!(fake_home.join(".claude/settings.json").exists());
    assert!(aenv_home.join("global-state.json").exists());

    // 2. Swap to default.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "activate", "default"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "swap to default failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"# Default mode"
    );
    assert_eq!(
        std::fs::read(fake_home.join(".claude/agents/explorer.md")).unwrap(),
        b"default agent"
    );

    // 3. status reports default as active.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "status"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global status failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("default"),
        "status stdout missing 'default': {stdout}"
    );

    // 4. which names default.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "which", "~/.claude/CLAUDE.md"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global which failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("default"),
        "which stdout missing 'default': {stdout}"
    );

    // 5. deactivate, then doctor --fix to clear the now-orphan stash.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global deactivate failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !aenv_home.join("global-state.json").exists(),
        "global-state.json should be removed after deactivate"
    );
    // Original CLAUDE.md restored.
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"original CLAUDE.md",
        "original CLAUDE.md not restored after deactivate"
    );
    // After a clean deactivate the stash it consumed is gone; any remaining
    // stash would be orphan. `doctor --fix` clears orphans and exits 0.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "doctor", "--fix"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global doctor --fix failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // Stash dir empty or absent.
    let stash_root = aenv_home.join("global-stash");
    if stash_root.exists() {
        let remaining: Vec<_> = std::fs::read_dir(&stash_root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        assert!(
            remaining.is_empty(),
            "global-stash should be empty after doctor --fix; got {} dirs",
            remaining.len()
        );
    }
}
