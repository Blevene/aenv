//! Deactivator runs `[lifecycle].on_deactivate` best-effort. Failure logs a
//! warning and continues with file restoration so the user is never left
//! stranded with materialized files they can't get rid of. `--force` skips
//! the lifecycle block entirely.
//!
//! Tempdir-as-fake-$HOME pattern, matching `lifecycle_activate.rs` so nothing
//! touches the developer's real environment.

use std::path::Path;

fn make_script(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }
}

/// Build the standard test scaffolding: a fake $HOME, a registry under
/// `<tmp>/.aenv`, the `claude-code` adapter, and a namespace `ns` with the
/// minimal user file at `user/.claude/CLAUDE.md`.
///
/// Returns `(aenv_home, fake_home, registry, adapters, ns_dir)`.
fn seed_namespace(
    tmp: &Path,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    aenv_core::home::RegistryLayout,
    aenv_core::adapter::AdapterRegistry,
    std::path::PathBuf,
) {
    let aenv_home = tmp.join(".aenv");
    let fake_home = tmp.join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.clone());
    let fs = aenv_core::fs::RealFilesystem;

    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new").unwrap();

    (aenv_home, fake_home, registry, adapters, ns_dir)
}

#[test]
fn on_deactivate_runs_during_normal_deactivation() {
    let tmp = tempfile::tempdir().unwrap();
    let (_aenv_home, fake_home, registry, adapters, ns_dir) = seed_namespace(tmp.path());
    let fs = aenv_core::fs::RealFilesystem;

    // on_activate succeeds (so lifecycle_ran=true); on_deactivate touches a
    // sentinel under fake_home.
    make_script(&ns_dir.join("ok.sh"), "#!/bin/sh\nexit 0\n");
    make_script(
        &ns_dir.join("bye.sh"),
        "#!/bin/sh\ntouch \"$AENV_TARGET_ROOT/.aenv-bye-ran\"\nexit 0\n",
    );
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "ok.sh"
on_deactivate = "bye.sh"
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap();
    assert!(
        !fake_home.join(".aenv-bye-ran").exists(),
        "on_deactivate must not run during activation"
    );

    aenv_core::deactivate::deactivate_namespace_in_scope_with_force(
        &fs,
        &registry,
        &fake_home,
        aenv_core::scope::Scope::User,
        false,
    )
    .unwrap();
    assert!(
        fake_home.join(".aenv-bye-ran").exists(),
        "on_deactivate should have run during deactivation"
    );
}

#[test]
fn on_deactivate_failure_does_not_block_file_restoration() {
    let tmp = tempfile::tempdir().unwrap();
    let (_aenv_home, fake_home, registry, adapters, ns_dir) = seed_namespace(tmp.path());
    let fs = aenv_core::fs::RealFilesystem;

    // Pre-existing user file gets stashed by activate, must be restored on
    // deactivate even if on_deactivate fails.
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original").unwrap();

    make_script(&ns_dir.join("ok.sh"), "#!/bin/sh\nexit 0\n");
    make_script(&ns_dir.join("boom.sh"), "#!/bin/sh\nexit 1\n");
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "ok.sh"
on_deactivate = "boom.sh"
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"new"
    );

    // Best-effort: failure here returns Ok, just prints a warning.
    let active = aenv_core::deactivate::deactivate_namespace_in_scope_with_force(
        &fs,
        &registry,
        &fake_home,
        aenv_core::scope::Scope::User,
        false,
    )
    .expect("on_deactivate failure must not bubble; restoration proceeds");
    assert_eq!(active, "ns");
    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"original",
        "pre-existing file must be restored despite on_deactivate failure"
    );
}

#[test]
fn force_skips_on_deactivate() {
    let tmp = tempfile::tempdir().unwrap();
    let (_aenv_home, fake_home, registry, adapters, ns_dir) = seed_namespace(tmp.path());
    let fs = aenv_core::fs::RealFilesystem;

    make_script(&ns_dir.join("ok.sh"), "#!/bin/sh\nexit 0\n");
    make_script(
        &ns_dir.join("bye.sh"),
        "#!/bin/sh\ntouch \"$AENV_TARGET_ROOT/.aenv-bye-ran\"\nexit 0\n",
    );
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "ok.sh"
on_deactivate = "bye.sh"
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap();

    aenv_core::deactivate::deactivate_namespace_in_scope_with_force(
        &fs,
        &registry,
        &fake_home,
        aenv_core::scope::Scope::User,
        true,
    )
    .unwrap();
    assert!(
        !fake_home.join(".aenv-bye-ran").exists(),
        "--force should skip on_deactivate entirely"
    );
}

#[test]
fn deactivate_without_lifecycle_ran_skips_on_deactivate() {
    let tmp = tempfile::tempdir().unwrap();
    let (_aenv_home, fake_home, registry, adapters, ns_dir) = seed_namespace(tmp.path());
    let fs = aenv_core::fs::RealFilesystem;

    // on_deactivate set but NO on_activate — activate produces
    // lifecycle_ran = false, so on_deactivate has nothing to undo and must
    // not fire.
    make_script(
        &ns_dir.join("bye.sh"),
        "#!/bin/sh\ntouch \"$AENV_TARGET_ROOT/.aenv-bye-ran\"\nexit 0\n",
    );
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_deactivate = "bye.sh"
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("ns").unwrap();
    let state = aenv_core::activate::activate_namespace_in_scope(
        &fs,
        &registry,
        &adapters,
        &fake_home,
        aenv_core::scope::Scope::User,
        &leaf,
    )
    .unwrap();
    assert!(!state.lifecycle_ran);

    aenv_core::deactivate::deactivate_namespace_in_scope_with_force(
        &fs,
        &registry,
        &fake_home,
        aenv_core::scope::Scope::User,
        false,
    )
    .unwrap();
    assert!(
        !fake_home.join(".aenv-bye-ran").exists(),
        "on_deactivate must not run when lifecycle_ran is false"
    );
}
