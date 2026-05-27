//! End-to-end tests for `aenv global doctor`.
//!
//! Drives the built `aenv` binary as a subprocess with `AENV_HOME` and `HOME`
//! pointed at a `tempfile::tempdir`. Exercises the user-scope doctor surface
//! in isolation from the real `$HOME`.

use std::path::Path;
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn global_doctor_reports_user_scope_oversize_instructions() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    // Tight user-scope soft-limit.
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]

[user_roles]
"~/.claude/CLAUDE.md" = "instructions"

[user_soft_limits]
instructions = 10
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/oversize");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), "x".repeat(500)).unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "oversize"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "doctor", "oversize"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("instructions_max_chars"),
        "stdout should name the policy: {stdout}"
    );
    assert!(
        stdout.contains("~/.claude/CLAUDE.md"),
        "stdout should name the ~/-prefixed target: {stdout}"
    );
    assert!(
        stdout.contains("[WARN]") || stdout.contains("[FAIL]"),
        "stdout should flag a violation: {stdout}"
    );
}

#[test]
fn global_doctor_clean_namespace_reports_no_issues() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/clean");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"small body").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "clean"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "doctor", "clean"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let has_violation = stdout.contains("[WARN]") || stdout.contains("[FAIL]");
    assert!(
        !has_violation || stdout.contains("No user-scope issues") || stdout.contains("no issues"),
        "expected no violations: {stdout}"
    );
}
