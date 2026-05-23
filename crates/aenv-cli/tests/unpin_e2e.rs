use std::process::Command;
use tempfile::TempDir;

fn setup_namespace(aenv_home: &std::path::Path, name: &str) {
    let envs = aenv_home.join(format!("envs/{name}"));
    std::fs::create_dir_all(&envs).unwrap();
    std::fs::write(
        envs.join("aenv.toml"),
        format!("name = \"{name}\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n"),
    )
    .unwrap();
    std::fs::write(envs.join("CLAUDE.md"), "# Hello\n").unwrap();
}

#[test]
fn unpin_idempotent_when_not_pinned() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let bin = env!("CARGO_BIN_EXE_aenv");
    let out = Command::new(bin)
        .args(["unpin", "--project", project.path().to_str().unwrap()])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("No namespace pinned"), "got: {stdout}");
}

#[test]
fn unpin_removes_pin_file_when_not_active() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    setup_namespace(aenv_home.path(), "solo");

    let bin = env!("CARGO_BIN_EXE_aenv");
    Command::new(bin)
        .args(["use", "solo", "--project", project.path().to_str().unwrap()])
        .env("AENV_HOME", aenv_home.path())
        .status()
        .unwrap();

    assert!(project.path().join(".aenv").exists());

    let out = Command::new(bin)
        .args(["unpin", "--project", project.path().to_str().unwrap()])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Unpinned"), "got: {stdout}");
    assert!(
        stdout.contains("solo"),
        "should name the unpinned namespace; got: {stdout}"
    );

    assert!(
        !project.path().join(".aenv").exists(),
        "pin file should be gone"
    );
}

#[test]
fn unpin_auto_deactivates_when_active() {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    setup_namespace(aenv_home.path(), "solo");

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

    assert!(project.path().join("CLAUDE.md").exists());
    assert!(project.path().join(".aenv-state").exists());

    let out = Command::new(bin)
        .args(["unpin", "--project", project.path().to_str().unwrap()])
        .env("AENV_HOME", aenv_home.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        !project.path().join(".aenv").exists(),
        "pin file should be gone"
    );
    assert!(
        !project.path().join("CLAUDE.md").exists(),
        "managed file should be gone"
    );
    assert!(
        !project.path().join(".aenv-state").exists(),
        "state dir should be scrubbed (deactivate already does this)"
    );
}
