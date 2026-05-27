//! End-to-end tests for `aenv global status` (text + JSON).
//!
//! Drives the built `aenv` binary as a subprocess with `AENV_HOME` and
//! `HOME` pointed at a `tempfile::tempdir`.

use std::path::Path;
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn global_status_without_activation_shows_inactive() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&aenv_home).unwrap();
    let out = aenv()
        .env("AENV_HOME", canon(&aenv_home))
        .env("HOME", canon(&fake_home))
        .args(["global", "status"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no global activation"),
        "expected 'no global activation' marker, got: {stdout}"
    );
}

#[test]
fn global_status_json_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = tmp.path().join(".aenv");
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"x").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let aenv_home = canon(&aenv_home);
    let fake_home = canon(&fake_home);
    aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "use", "ns"])
        .status()
        .unwrap();
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "status", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
        panic!("not JSON: {}", String::from_utf8_lossy(&out.stdout));
    });
    assert_eq!(json["scope"], "user");
    assert_eq!(json["active"], true);
    assert_eq!(json["active_namespace"], "ns");
    let managed = json["managed_files"]
        .as_array()
        .expect("managed_files array");
    assert_eq!(managed.len(), 1);
    assert_eq!(managed[0]["path"], ".claude/CLAUDE.md");
}
