//! Functional spec §7.5 — scripted comparison.
//!
//! Three namespaces with distinct content. The script activates each in
//! turn against the same project, reads `aenv status --json`, and
//! captures the resolved_hash plus the list of managed-file short names.
//! Asserts the hashes are distinct (different namespaces → different
//! material) and that re-activating the same namespace is hash-stable.

use std::collections::HashSet;
use std::process::Command;
use tempfile::TempDir;

fn write(dir: &std::path::Path, relpath: &str, body: &str) {
    let p = dir.join(relpath);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

#[test]
fn three_namespaces_produce_three_distinct_hashes() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let envs = aenv_home.path().join("envs");

    write(
        &envs,
        "experiments/aenv.toml",
        "name = \"experiments\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write(&envs, "experiments/CLAUDE.md", "# Experiments\nBe broad.\n");

    write(
        &envs,
        "detailed/aenv.toml",
        "name = \"detailed\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write(&envs, "detailed/CLAUDE.md", "# Detailed\nBe careful.\n");

    write(
        &envs,
        "analyst/aenv.toml",
        "name = \"analyst\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write(&envs, "analyst/CLAUDE.md", "# Analyst\nRead-only.\n");

    let bin = env!("CARGO_BIN_EXE_aenv");
    let mut hashes: Vec<String> = Vec::new();
    for ns in ["experiments", "detailed", "analyst"] {
        Command::new(bin)
            .args(["deactivate", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path())
            .status()
            .ok();
        Command::new(bin)
            .args(["use", ns, "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path())
            .status()
            .unwrap();
        Command::new(bin)
            .args(["activate", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path())
            .status()
            .unwrap();
        let out = Command::new(bin)
            .args([
                "status",
                "--project",
                project.path().to_str().unwrap(),
                "--json",
            ])
            .env("AENV_HOME", aenv_home.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "{ns} status failed");
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let h = v["resolved_hash"].as_str().unwrap().to_string();
        assert!(h.starts_with("sha256-v1:"));
        hashes.push(h);
    }

    let unique: HashSet<&String> = hashes.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "expected three distinct hashes, got {hashes:?}"
    );
}

#[test]
fn reactivating_same_namespace_is_hash_stable() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let envs = aenv_home.path().join("envs/stable");
    std::fs::create_dir_all(&envs).unwrap();
    std::fs::write(
        envs.join("aenv.toml"),
        "name = \"stable\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(envs.join("CLAUDE.md"), "# Stable\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_aenv");
    let mut hashes: Vec<String> = Vec::new();
    for _ in 0..2 {
        Command::new(bin)
            .args(["deactivate", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path())
            .status()
            .ok();
        Command::new(bin)
            .args([
                "use",
                "stable",
                "--project",
                project.path().to_str().unwrap(),
            ])
            .env("AENV_HOME", aenv_home.path())
            .status()
            .unwrap();
        Command::new(bin)
            .args(["activate", "--project", project.path().to_str().unwrap()])
            .env("AENV_HOME", aenv_home.path())
            .status()
            .unwrap();
        let out = Command::new(bin)
            .args([
                "status",
                "--project",
                project.path().to_str().unwrap(),
                "--json",
            ])
            .env("AENV_HOME", aenv_home.path())
            .output()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        hashes.push(v["resolved_hash"].as_str().unwrap().to_string());
    }
    assert_eq!(
        hashes[0], hashes[1],
        "re-activating same namespace must be hash-stable"
    );
}
