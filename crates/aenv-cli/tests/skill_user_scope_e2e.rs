//! End-to-end test for user-scope skills in a global profile.
//!
//! Regression guard: `[[skills]] scope = "user"` must materialize under the
//! adapter's `user_skills_dir` (`~/.claude/skills/<name>/`) on `aenv global
//! use`. Before the fix, the resolver hardcoded skill candidates to project
//! scope, so they never reached a global activation.

use std::path::Path;
use std::process::Command;

fn aenv(aenv_home: &Path, fake_home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
    c.env("AENV_HOME", aenv_home).env("HOME", fake_home);
    c
}

fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn user_scope_authored_skill_materializes_globally() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();

    // Adapter declares both project and user skills dirs.
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        "name = \"claude-code\"\nuser_files = [\"~/.claude/CLAUDE.md\"]\nskills_dir = \".claude/skills\"\nuser_skills_dir = \"~/.claude/skills\"\n",
    )
    .unwrap();

    // A global namespace.
    let ns = aenv_home.join("envs/research");
    std::fs::create_dir_all(ns.join("user/.claude")).unwrap();
    std::fs::write(ns.join("user/.claude/CLAUDE.md"), b"# research").unwrap();
    std::fs::write(
        ns.join("aenv.toml"),
        "name = \"research\"\n[adapters.claude-code]\nuser_files = [\".claude/CLAUDE.md\"]\n",
    )
    .unwrap();

    // Scaffold a user-scope authored skill.
    let out = aenv(&aenv_home, &fake_home)
        .args([
            "skill", "new", "explorer", "--ns", "research", "--scope", "user",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "skill new --scope user failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Authored user-scope source lives under the namespace's user/ subtree.
    assert!(
        ns.join("user/.claude/skills/explorer/SKILL.md").exists(),
        "user-scope skill not scaffolded under user/.claude/skills/"
    );

    // Activate globally; the skill must land under $HOME/.claude/skills/.
    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "use", "research", "--yes", "--no-baseline"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global use failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        fake_home.join(".claude/skills/explorer/SKILL.md").exists(),
        "user-scope skill did not materialize under ~/.claude/skills/ (stdout: {})",
        String::from_utf8_lossy(&out.stdout)
    );
    // The instructions file came along too.
    assert!(fake_home.join(".claude/CLAUDE.md").exists());
}

#[test]
fn invalid_scope_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        "name = \"claude-code\"\nskills_dir = \".claude/skills\"\n",
    )
    .unwrap();
    let ns = aenv_home.join("envs/x");
    std::fs::create_dir_all(&ns).unwrap();
    std::fs::write(
        ns.join("aenv.toml"),
        "name = \"x\"\n[adapters.claude-code]\nfiles = []\n",
    )
    .unwrap();

    let out = aenv(&aenv_home, &fake_home)
        .args(["skill", "new", "s", "--ns", "x", "--scope", "bogus"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "bogus --scope should fail");
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid --scope"));
}
