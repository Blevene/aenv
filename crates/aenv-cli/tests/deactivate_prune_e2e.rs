//! End-to-end test for `aenv deactivate --prune` — the project-side
//! analog of `aenv global deactivate --prune`. Verifies that
//! `.aenv-state/backup/<ts>/` directories are removed only when `--prune`
//! is passed.

use std::path::Path;
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}
fn canon(p: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn deactivate_prune_removes_backup_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let project = canon(tmp.path()).join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/ns");
    std::fs::create_dir_all(&ns_dir).unwrap();
    std::fs::write(ns_dir.join("CLAUDE.md"), b"new body").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    )
    .unwrap();

    // Create a pre-existing CLAUDE.md that aenv will back up.
    std::fs::write(project.join("CLAUDE.md"), b"original body").unwrap();

    // use + activate the namespace — produces one .aenv-state/backup/<ts>/ dir.
    aenv()
        .env("AENV_HOME", &aenv_home)
        .args(["use", "ns", "--project", project.to_str().unwrap()])
        .status()
        .unwrap();
    aenv()
        .env("AENV_HOME", &aenv_home)
        .args(["activate", "--project", project.to_str().unwrap()])
        .status()
        .unwrap();

    let backup_root = project.join(".aenv-state/backup");
    let pre_prune_count = std::fs::read_dir(&backup_root).unwrap().count();
    assert!(
        pre_prune_count > 0,
        "expected at least one backup dir before deactivate"
    );

    // Deactivate without --prune leaves the backups behind.
    aenv()
        .env("AENV_HOME", &aenv_home)
        .args(["deactivate", "--project", project.to_str().unwrap()])
        .status()
        .unwrap();
    let post_deactivate_count = if backup_root.exists() {
        std::fs::read_dir(&backup_root).unwrap().count()
    } else {
        0
    };
    assert_eq!(
        post_deactivate_count, pre_prune_count,
        "deactivate without --prune should preserve backup dirs"
    );

    // Re-activate and deactivate --prune; backups should be gone.
    aenv()
        .env("AENV_HOME", &aenv_home)
        .args(["activate", "--project", project.to_str().unwrap()])
        .status()
        .unwrap();
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .args([
            "deactivate",
            "--prune",
            "--project",
            project.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "deactivate --prune failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Pruned"),
        "stdout should mention pruning: {stdout}"
    );

    // Backup root is either gone or empty.
    if backup_root.exists() {
        let remaining: Vec<_> = std::fs::read_dir(&backup_root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        assert!(
            remaining.is_empty(),
            ".aenv-state/backup should be empty after --prune; got {} dirs",
            remaining.len()
        );
    }
}
