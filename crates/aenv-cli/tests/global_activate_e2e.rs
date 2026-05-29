//! End-to-end tests for `aenv global activate` and `aenv global deactivate`.
//!
//! Drives the built `aenv` binary as a subprocess with `AENV_HOME` and
//! `HOME` pointed at a `tempfile::tempdir`. Exercises the user-scope
//! activation surface in isolation from the real `$HOME`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_aenv").into()
}

fn aenv(aenv_home: &Path, fake_home: &Path) -> Command {
    let mut c = Command::new(bin());
    c.env("AENV_HOME", aenv_home).env("HOME", fake_home);
    c
}

/// Set executable bits on a unix script. No-op elsewhere.
fn make_executable(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(_path, perms).unwrap();
    }
}

/// Seed a namespace `ns` with an `on_activate` script body of `script_body`
/// (writes it to `ns/ok.sh` and references it from the manifest). The
/// caller pre-creates `aenv_home`.
fn seed_namespace_with_on_activate(aenv_home: &Path, script_body: &str) {
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new").unwrap();
    let script = ns_dir.join("ok.sh");
    std::fs::write(&script, script_body).unwrap();
    make_executable(&script);
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "ok.sh"
"#,
    )
    .unwrap();
}

/// Create the minimal `claude-code` adapter (with a single `~/.claude/CLAUDE.md`
/// user file) plus a namespace `ns` whose `user/.claude/CLAUDE.md` payload is
/// the literal bytes `new`.
fn seed_minimal_user_scope(aenv_home: &Path) {
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/ns");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"new").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "ns"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();
}

#[test]
fn global_activate_materializes_user_files_under_home_override() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_minimal_user_scope(&aenv_home);

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global activate failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let materialized = fake_home.join(".claude/CLAUDE.md");
    assert!(materialized.exists(), "missing {materialized:?}");
    assert_eq!(std::fs::read(&materialized).unwrap(), b"new");
    assert!(aenv_home.join("global-state.json").exists());

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ns"),
        "stdout did not mention namespace name: {stdout}"
    );
    assert!(
        stdout.contains("running harness sessions"),
        "stdout missing running-session caveat: {stdout}"
    );
}

#[test]
fn global_deactivate_restores_stash() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(fake_home.join(".claude")).unwrap();
    std::fs::write(fake_home.join(".claude/CLAUDE.md"), b"original").unwrap();
    seed_minimal_user_scope(&aenv_home);

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global activate failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global deactivate failed: status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert_eq!(
        std::fs::read(fake_home.join(".claude/CLAUDE.md")).unwrap(),
        b"original",
        "original CLAUDE.md not restored after deactivate"
    );
    assert!(
        !aenv_home.join("global-state.json").exists(),
        "global-state.json should be removed after deactivate"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ns"),
        "deactivate stdout should mention namespace name: {stdout}"
    );
}

#[test]
fn use_with_global_flag_activates_both_scopes() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    let project = std::fs::canonicalize(tmp.path()).unwrap().join("project");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
files = ["CLAUDE.md"]
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/both");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("CLAUDE.md"), b"project body").unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"user body").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "both"
[adapters.claude-code]
files = ["CLAUDE.md"]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let out = aenv(&aenv_home, &fake_home)
        .args([
            "use",
            "both",
            "--global",
            "--project",
            project.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // `aenv use --global` is sugar for: pin the project, activate it,
    // and activate globally. All three side effects must land.
    assert!(project.join(".aenv").exists(), "project not pinned");

    // Project-scope materialization: CLAUDE.md should exist under the
    // project root (this is the activate step that previously was missing).
    let project_claude = project.join("CLAUDE.md");
    assert!(
        project_claude.exists(),
        "project CLAUDE.md not materialized by --global sugar: {project_claude:?}"
    );
    assert_eq!(std::fs::read(&project_claude).unwrap(), b"project body");

    // User-scope materialization: $HOME/.claude/CLAUDE.md.
    let user_claude = fake_home.join(".claude/CLAUDE.md");
    assert!(
        user_claude.exists(),
        "user CLAUDE.md not materialized: {user_claude:?}"
    );
    assert_eq!(std::fs::read(&user_claude).unwrap(), b"user body");
    assert!(aenv_home.join("global-state.json").exists());
}

#[test]
fn global_deactivate_with_nothing_active_is_ok() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global deactivate with no activation should succeed: status={:?}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no global activation"),
        "expected no-op message, got: {stdout}"
    );
}

// --------------------------------------------------------------------------
// `--yes` / SHA-pinned approval marker
// --------------------------------------------------------------------------

#[test]
fn activate_with_yes_writes_approval_marker() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_namespace_with_on_activate(&aenv_home, "#!/bin/sh\nexit 0\n");

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns", "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "global activate --yes failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let marker = aenv_home.join("envs/ns/.approved");
    assert!(marker.exists(), ".approved marker not written: {marker:?}");
    let body = std::fs::read_to_string(&marker).unwrap();
    assert!(
        body.trim_start().starts_with("sha256:"),
        "marker body should start with sha256:, got {body:?}"
    );

    // The recorded sha must match the script's actual hash.
    let script_bytes = std::fs::read(aenv_home.join("envs/ns/ok.sh")).unwrap();
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(&script_bytes);
    let expected_hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let expected = format!("sha256:{expected_hex}");
    assert_eq!(body.trim(), expected, "marker sha mismatch");
}

#[test]
fn second_activation_with_unchanged_script_does_not_reprompt() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_namespace_with_on_activate(&aenv_home, "#!/bin/sh\nexit 0\n");

    // First activation records the approval.
    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns", "--yes"])
        .output()
        .unwrap();
    assert!(out.status.success());

    // Deactivate so the state file is gone.
    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(out.status.success(), "deactivate failed");

    // Second activation WITHOUT --yes. We give it an empty stdin so any
    // attempt to prompt would yield "n" and abort. Success means the
    // marker short-circuited the prompt.
    let mut child = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "second activation should succeed silently: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("Aborted"),
        "unchanged script must not trigger the prompt path"
    );
    assert!(stdout.contains("Activated"));
}

#[test]
fn script_change_invalidates_approval() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_namespace_with_on_activate(&aenv_home, "#!/bin/sh\nexit 0\n");

    // Approve v1.
    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns", "--yes"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "deactivate"])
        .output()
        .unwrap();
    assert!(out.status.success());

    // Mutate the script. Re-set perms because some FSes drop them on
    // overwrite. The previous approval is now stale (sha mismatch).
    let script = aenv_home.join("envs/ns/ok.sh");
    std::fs::write(&script, "#!/bin/sh\n# changed\nexit 0\n").unwrap();
    make_executable(&script);

    // Activate without --yes, declining the prompt with "n".
    let mut child = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"n\n").unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "declining a script change is not an error: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Aborted") || stdout.contains("not re-approved"),
        "expected re-approval prompt, got: {stdout}"
    );

    // No global activation was created — the abort happens before swap.
    assert!(
        !aenv_home.join("global-state.json").exists(),
        "declined re-approval should not have activated anything"
    );
}

#[test]
fn activate_without_yes_no_stdin_returns_aborted() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_namespace_with_on_activate(&aenv_home, "#!/bin/sh\nexit 0\n");

    // No prior approval, closed stdin -> prompt reads "" -> declines.
    let mut child = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "ns"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "decline is not an error: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Aborted"),
        "expected 'Aborted' message, got: {stdout}"
    );

    // The marker is not created on decline.
    assert!(
        !aenv_home.join("envs/ns/.approved").exists(),
        "declined approval should not write a marker"
    );
    // And nothing was activated.
    assert!(!aenv_home.join("global-state.json").exists());
}

#[test]
fn global_deactivate_force_skips_failing_on_deactivate() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let ns_dir = aenv_home.join("envs/lifecycle-bad");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"x").unwrap();
    let ok = ns_dir.join("ok.sh");
    let fail = ns_dir.join("fail.sh");
    std::fs::write(&ok, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::write(
        &fail,
        "#!/bin/sh\ntouch \"$AENV_TARGET_ROOT/.deactivate-ran\"\nexit 1\n",
    )
    .unwrap();
    make_executable(&ok);
    make_executable(&fail);
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "lifecycle-bad"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]

[lifecycle]
on_activate = "ok.sh"
on_deactivate = "fail.sh"
"#,
    )
    .unwrap();

    let act = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "lifecycle-bad", "--yes"])
        .output()
        .unwrap();
    assert!(
        act.status.success(),
        "activate failed: {}",
        String::from_utf8_lossy(&act.stderr)
    );

    // With --force, on_deactivate must be skipped, so the sentinel is not
    // touched. File restoration still completes.
    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "deactivate", "--force"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "deactivate --force failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--force: skipped on_deactivate"),
        "expected --force note in stdout, got: {stdout}"
    );

    assert!(
        !fake_home.join(".deactivate-ran").exists(),
        "on_deactivate ran despite --force"
    );
    assert!(
        !aenv_home.join("global-state.json").exists(),
        "state file should be gone after deactivate"
    );
}

// --------------------------------------------------------------------------
// Pre-flight scan (--yes interaction, decline)
// --------------------------------------------------------------------------

/// Seed a namespace whose settings.json references a hook command at a
/// path that doesn't exist anywhere on disk and isn't being materialized.
/// Use this to drive the pre-flight prompt.
fn seed_namespace_with_broken_hook(aenv_home: &Path) {
    std::fs::create_dir_all(aenv_home.join("adapters")).unwrap();
    std::fs::write(
        aenv_home.join("adapters/claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/settings.json"]
"#,
    )
    .unwrap();
    let ns_dir = aenv_home.join("envs/brokens");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(
        ns_dir.join("user/.claude/settings.json"),
        br#"{
            "hooks": {
                "PreToolUse": [
                    { "hooks": [ { "type": "command",
                        "command": "/definitely/not/here/policy.sh" } ] }
                ]
            }
        }"#,
    )
    .unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "brokens"
[adapters.claude-code]
user_files = [".claude/settings.json"]
"#,
    )
    .unwrap();
}

#[test]
fn activate_with_yes_proceeds_silently_through_preflight() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_namespace_with_broken_hook(&aenv_home);

    let out = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "brokens", "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "activate --yes (with pre-flight findings) failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The pre-flight banner SHOULD appear (we want users to see what they
    // approved with --yes) but the activation succeeds without prompting.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Pre-flight found"),
        "pre-flight banner missing under --yes: {stderr}"
    );
    assert!(
        stderr.contains("/definitely/not/here/policy.sh"),
        "missing path not surfaced: {stderr}"
    );
    assert!(aenv_home.join("global-state.json").exists());
}

#[test]
fn activate_without_yes_no_stdin_aborts_on_preflight_findings() {
    let tmp = tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(tmp.path()).unwrap().join(".aenv");
    let fake_home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&aenv_home).unwrap();
    std::fs::create_dir_all(&fake_home).unwrap();
    seed_namespace_with_broken_hook(&aenv_home);

    let mut child = aenv(&aenv_home, &fake_home)
        .args(["global", "activate", "brokens"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "declining pre-flight is not an error: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Aborted: pre-flight not approved"),
        "expected pre-flight abort message, got: {stdout}"
    );
    // No activation landed.
    assert!(!aenv_home.join("global-state.json").exists());
}
