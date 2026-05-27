//! End-to-end tests for `aenv global which | list | diff` — the read-only
//! verbs of the global subcommand tree.

use std::path::Path;
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}
fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

fn setup_active_ns(aenv_home: &Path, fake_home: &Path, ns_name: &str, body: &[u8]) {
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs").join(ns_name);
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), body).unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        format!(
            r#"
name = "{ns_name}"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#
        ),
    )
    .unwrap();
    aenv()
        .env("AENV_HOME", aenv_home)
        .env("HOME", fake_home)
        .args(["global", "use", ns_name])
        .status()
        .unwrap();
}

#[test]
fn global_which_returns_managing_namespace() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    setup_active_ns(&aenv_home, &fake_home, "ns", b"x");

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "which", "~/.claude/CLAUDE.md"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ns"), "stdout: {stdout}");
    assert!(stdout.contains("CLAUDE.md"), "stdout: {stdout}");
}

#[test]
fn global_list_filters_to_namespaces_with_user_files() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    // Namespace 1: declares user_files
    let ns1 = aenv_home.join("envs/with-user");
    std::fs::create_dir_all(ns1.join("user/.claude")).unwrap();
    std::fs::write(ns1.join("user/.claude/CLAUDE.md"), b"x").unwrap();
    std::fs::write(
        ns1.join("aenv.toml"),
        r#"
name = "with-user"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    // Namespace 2: only project files
    let ns2 = aenv_home.join("envs/only-project");
    std::fs::create_dir_all(&ns2).unwrap();
    std::fs::write(ns2.join("CLAUDE.md"), b"x").unwrap();
    std::fs::write(
        ns2.join("aenv.toml"),
        r#"
name = "only-project"
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    )
    .unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .args(["global", "list"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("with-user"));
    assert!(
        !stdout.contains("only-project"),
        "should not list only-project: {stdout}"
    );
}

#[test]
fn global_diff_drift_runs_without_crashing() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    setup_active_ns(&aenv_home, &fake_home, "ns", b"original");

    // Modify the materialized file in $HOME to introduce drift.
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"edited locally").unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "diff"])
        .output()
        .unwrap();
    // Whether the diff reports drift or not, the command must not crash
    // with a non-zero exit.
    assert!(
        out.status.success() || out.status.code() == Some(0),
        "diff crashed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}
