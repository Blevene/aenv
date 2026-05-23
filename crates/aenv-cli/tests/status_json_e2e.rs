//! End-to-end: aenv status --json produces parseable JSON with the
//! top-level keys functional spec §7.1 documents.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

struct Harness {
    _aenv_home_guard: tempfile::TempDir,
    _project_guard: tempfile::TempDir,
    aenv_home: PathBuf,
    project: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let aenv_home_guard = tempdir().unwrap();
        let project_guard = tempdir().unwrap();
        let aenv_home = std::fs::canonicalize(aenv_home_guard.path()).unwrap();
        let project = std::fs::canonicalize(project_guard.path()).unwrap();
        Self {
            _aenv_home_guard: aenv_home_guard,
            _project_guard: project_guard,
            aenv_home,
            project,
        }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_aenv"));
        c.env("AENV_HOME", &self.aenv_home);
        c
    }

    fn aenv_home(&self) -> &Path {
        &self.aenv_home
    }

    fn project(&self) -> &Path {
        &self.project
    }
}

fn assert_success(out: &std::process::Output, ctx: &str) {
    if !out.status.success() {
        panic!(
            "{ctx} failed: status={:?}, stdout={}, stderr={}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn status_json_against_active_project() {
    let h = Harness::new();

    let envs_dir = h.aenv_home().join("envs/solo");
    std::fs::create_dir_all(&envs_dir).unwrap();
    std::fs::write(
        envs_dir.join("aenv.toml"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(envs_dir.join("CLAUDE.md"), "# Hello\n").unwrap();

    let out = h
        .cmd()
        .args(["use", "solo", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "aenv use solo");

    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "aenv activate");

    let out = h
        .cmd()
        .args(["status", "--project"])
        .arg(h.project())
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "status --json failed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is valid JSON");
    assert!(v["project"].is_string(), "project must be a string");
    assert_eq!(v["active_namespace"], "solo");
    assert!(
        v["resolution_chain"].is_array(),
        "resolution_chain must be an array"
    );
    assert!(
        v["resolved_hash"]
            .as_str()
            .unwrap_or_default()
            .starts_with("sha256-v1:"),
        "resolved_hash must start with sha256-v1:"
    );
    assert!(
        v["managed_files"].is_array(),
        "managed_files must be an array"
    );
    assert!(v["backed_up"].is_array(), "backed_up must be an array");
}

#[test]
fn status_json_unpinned_project() {
    let h = Harness::new();

    let out = h
        .cmd()
        .args(["status", "--project"])
        .arg(h.project())
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "status --json (unpinned) failed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is valid JSON");
    assert!(v["project"].is_string(), "project must be a string");
    // active_namespace is null/absent when unpinned
    assert!(
        v["active_namespace"].is_null() || !v.as_object().unwrap().contains_key("active_namespace"),
        "active_namespace should be null or absent when unpinned"
    );
}
