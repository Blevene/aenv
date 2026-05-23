use std::process::Command;
use tempfile::TempDir;

#[test]
fn diff_no_args_reports_clean_on_freshly_activated_project() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let envs = aenv_home.path().join("envs/solo");
    std::fs::create_dir_all(&envs).unwrap();
    std::fs::write(
        envs.join("aenv.toml"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(envs.join("CLAUDE.md"), "# Hi\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_aenv");
    Command::new(bin)
        .args(["use", "solo", "--project", project.path().to_str().unwrap()])
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
            "diff",
            "--project",
            project.path().to_str().unwrap(),
            "--json",
        ])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["drifted"].as_array().unwrap().is_empty());
}

#[test]
fn diff_two_namespaces_reports_skill_added() {
    let aenv_home = TempDir::new().unwrap();
    for (name, body) in [
        ("alpha", "name = \"alpha\"\n[adapters.claude-code]\nfiles = []\n"),
        (
            "beta",
            "name = \"beta\"\n[adapters.claude-code]\nfiles = []\n[[skills]]\nname = \"new\"\nmode = \"authored\"\nadapter = \"claude-code\"\n",
        ),
    ] {
        let envs = aenv_home.path().join(format!("envs/{name}"));
        std::fs::create_dir_all(&envs).unwrap();
        std::fs::write(envs.join("aenv.toml"), body).unwrap();
    }

    let bin = env!("CARGO_BIN_EXE_aenv");
    let out = Command::new(bin)
        .args(["diff", "alpha", "beta", "--json"])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["a"], "alpha");
    assert_eq!(v["b"], "beta");
    assert!(v["skills"]["added"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == "beta::new"));
}
