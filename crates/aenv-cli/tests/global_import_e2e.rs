//! End-to-end tests for `aenv global import` (local-path source).
//!
//! Drives the built `aenv` binary as a subprocess with `AENV_HOME` and
//! `HOME` pointed at `tempfile::tempdir`. Git URL coverage lives in
//! `global_import_git_e2e.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn aenv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aenv"))
}

fn canon(p: impl AsRef<Path>) -> PathBuf {
    std::fs::canonicalize(p.as_ref()).unwrap()
}

#[test]
fn global_import_local_dir_creates_activable_namespace() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    // Lay down a claude-ctrl-style source tree.
    let src = canon(tmp.path()).join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("CLAUDE.md"), b"# top").unwrap();
    std::fs::create_dir_all(src.join("agents")).unwrap();
    std::fs::write(src.join("agents/a.md"), b"agent a").unwrap();
    std::fs::create_dir_all(src.join("hooks")).unwrap();
    std::fs::write(src.join("hooks/pre.sh"), b"#!/bin/sh\n").unwrap();
    std::fs::write(src.join("install.sh"), b"#!/bin/sh\necho install\n").unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "import"])
        .arg(&src)
        .arg("claude-cntrl")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "import failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let ns_dir = aenv_home.join("envs/claude-cntrl");
    assert!(ns_dir.join("aenv.toml").exists());
    assert!(ns_dir.join("install.sh").exists());
    assert!(ns_dir.join("user/.claude/CLAUDE.md").exists());
    assert_eq!(
        std::fs::read(ns_dir.join("user/.claude/CLAUDE.md")).unwrap(),
        b"# top"
    );
    assert!(ns_dir.join("user/.claude/agents/a.md").exists());
    assert!(ns_dir.join("user/.claude/hooks/pre.sh").exists());

    // The manifest should mention the [lifecycle] section.
    let manifest_text = std::fs::read_to_string(ns_dir.join("aenv.toml")).unwrap();
    assert!(
        manifest_text.contains("[lifecycle]"),
        "expected [lifecycle] in manifest:\n{manifest_text}"
    );

    // `aenv global list` surfaces the new namespace.
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("claude-cntrl"),
        "expected claude-cntrl in list output:\n{stdout}"
    );
}

#[test]
fn global_import_default_name_from_source_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let src = canon(tmp.path()).join("my-handle");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("CLAUDE.md"), b"hi").unwrap();

    // No name argument — should default to "my-handle".
    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "import"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "import failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(aenv_home.join("envs/my-handle/aenv.toml").exists());
    assert!(aenv_home
        .join("envs/my-handle/user/.claude/CLAUDE.md")
        .exists());
}

#[test]
fn global_import_with_convention_file_uses_explicit_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let src = canon(tmp.path()).join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("aenv-namespace.toml"),
        br#"adapters = ["claude-code"]

[layout]
"myrules/" = ".claude/myrules/"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(src.join("myrules")).unwrap();
    std::fs::write(src.join("myrules/a.md"), b"a").unwrap();
    std::fs::write(src.join("myrules/b.md"), b"b").unwrap();
    // Files NOT in [layout] should be ignored.
    std::fs::write(src.join("README.md"), b"readme - skip me").unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "import"])
        .arg(&src)
        .arg("conv-ns")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "import failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let ns_dir = aenv_home.join("envs/conv-ns");
    assert!(ns_dir.join("user/.claude/myrules/a.md").exists());
    assert!(ns_dir.join("user/.claude/myrules/b.md").exists());
    // README.md was not in the layout, so it should not appear anywhere.
    assert!(!ns_dir.join("README.md").exists());
    assert!(!ns_dir.join("user/README.md").exists());

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("aenv-namespace.toml convention file"),
        "expected stdout to mention the convention file, got:\n{stdout}"
    );
}

#[test]
fn global_import_rejects_pin_for_local_source() {
    let tmp = tempfile::tempdir().unwrap();
    let aenv_home = canon(tmp.path()).join(".aenv");
    let fake_home = canon(tmp.path()).join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let src = canon(tmp.path()).join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("CLAUDE.md"), b"x").unwrap();

    let out = aenv()
        .env("AENV_HOME", &aenv_home)
        .env("HOME", &fake_home)
        .args(["global", "import"])
        .arg(&src)
        .arg("ns")
        .args(["--pin", "v1"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "expected --pin on a local path to fail"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--pin only applies to git URL"),
        "expected stderr to explain --pin scope, got: {stderr}"
    );
}
