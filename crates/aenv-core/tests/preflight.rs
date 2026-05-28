//! Integration tests for `aenv_core::preflight`.
//!
//! These exercise the settings.json command-path scanner against synthetic
//! candidates whose source files live in a real `tempfile::tempdir` (we use
//! `RealFilesystem` so `fs.exists()` on a freshly-created path returns true,
//! letting the materialization-set test cover that branch).

use aenv_core::fs::RealFilesystem;
use aenv_core::identity::NamespaceId;
use aenv_core::preflight::{preflight_settings_commands, PreflightKind};
use aenv_core::resolve::Candidate;
use aenv_core::scope::Scope;
use std::path::{Path, PathBuf};

fn ns(name: &str) -> NamespaceId {
    NamespaceId::new(name).unwrap()
}

/// Write `bytes` to `source_path` and return a `Candidate` whose target path
/// (relative to the activation target) is `target_rel` and whose source is
/// the written file.
fn settings_candidate(source_path: &Path, target_rel: &str, bytes: &[u8]) -> Candidate {
    if let Some(parent) = source_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(source_path, bytes).unwrap();
    Candidate {
        namespace: ns("ns"),
        path: PathBuf::from(target_rel),
        source_path: source_path.to_path_buf(),
        adapter: "claude-code".into(),
        scope: Scope::User,
        merge_override: None,
        skill_provenance: None,
        adapter_materialize_override: None,
    }
}

#[test]
fn preflight_flags_missing_hook_command() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    let settings = br#"{
        "hooks": {
            "PreToolUse": [
                { "hooks": [ { "type": "command", "command": "/definitely/not/here/policy.sh" } ] }
            ]
        }
    }"#;
    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", settings);

    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert!(matches!(&f.kind, PreflightKind::Hook { event } if event == "PreToolUse"));
    assert_eq!(
        f.missing_path,
        PathBuf::from("/definitely/not/here/policy.sh")
    );
    assert_eq!(f.command, "/definitely/not/here/policy.sh");
}

#[test]
fn preflight_flags_missing_mcp_server() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    let settings = br#"{
        "mcpServers": {
            "foo": { "command": "/nowhere/server.py" }
        }
    }"#;
    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", settings);

    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    assert_eq!(findings.len(), 1);
    assert!(matches!(&findings[0].kind, PreflightKind::McpServer { name } if name == "foo"));
}

#[test]
fn preflight_flags_missing_status_line() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    let settings = br#"{
        "statusLine": { "type": "command", "command": "/nope/statusline.sh" }
    }"#;
    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", settings);

    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    assert_eq!(findings.len(), 1);
    assert!(matches!(findings[0].kind, PreflightKind::StatusLine));
}

#[test]
fn preflight_skips_bare_binaries_in_path() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    let settings = br#"{
        "hooks": {
            "PreToolUse": [
                { "hooks": [ { "type": "command", "command": "python3 some-script.py" } ] }
            ]
        }
    }"#;
    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", settings);

    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    assert!(
        findings.is_empty(),
        "bare binary should be skipped: {findings:?}"
    );
}

#[test]
fn preflight_skips_inline_shell_expressions() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    let settings = br#"{
        "hooks": {
            "PreToolUse": [
                {
                    "hooks": [
                        { "type": "command",
                          "command": "{ cat; echo; } >> $HOME/.claude/runtime/log.jsonl" }
                    ]
                }
            ]
        }
    }"#;
    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", settings);

    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    assert!(
        findings.is_empty(),
        "inline shell should be skipped: {findings:?}"
    );
}

#[test]
fn preflight_skips_paths_being_materialized_this_run() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    // The hook references $HOME/.claude/runtime/cli.py — a path that doesn't
    // exist on disk but WILL be materialized by an in-this-run candidate.
    let settings = br#"{
        "hooks": {
            "PreToolUse": [
                { "hooks": [ { "type": "command",
                    "command": "$HOME/.claude/runtime/cli.py" } ] }
            ]
        }
    }"#;
    let settings_source = tmp.path().join("ns/.claude/settings.json");
    let runtime_source = tmp.path().join("ns/.claude/runtime/cli.py");
    let settings_candidate_obj =
        settings_candidate(&settings_source, ".claude/settings.json", settings);
    // Build a second candidate whose target IS the runtime path the hook
    // references — same activation will materialize it, so pre-flight must
    // suppress the finding.
    std::fs::create_dir_all(runtime_source.parent().unwrap()).unwrap();
    std::fs::write(&runtime_source, b"#!/usr/bin/env python3\n").unwrap();
    let runtime_candidate = Candidate {
        namespace: ns("ns"),
        path: PathBuf::from(".claude/runtime/cli.py"),
        source_path: runtime_source,
        adapter: "claude-code".into(),
        scope: Scope::User,
        merge_override: None,
        skill_provenance: None,
        adapter_materialize_override: None,
    };

    let findings = preflight_settings_commands(
        &RealFilesystem,
        &target_root,
        &[settings_candidate_obj, runtime_candidate],
    )
    .unwrap();
    assert!(
        findings.is_empty(),
        "path being materialized this run should be suppressed: {findings:?}"
    );
}

#[test]
fn preflight_handles_malformed_settings_json_gracefully() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", b"{ this is not json");

    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    assert!(
        findings.is_empty(),
        "malformed JSON must not produce findings"
    );
}

#[test]
fn preflight_resolves_home_env_var() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = std::fs::canonicalize(tmp.path()).unwrap().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    // First: $HOME/script.sh doesn't exist -> 1 finding.
    let settings = br#"{
        "hooks": {
            "PreToolUse": [
                { "hooks": [ { "type": "command", "command": "$HOME/script.sh" } ] }
            ]
        }
    }"#;
    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", settings);

    let findings = preflight_settings_commands(
        &RealFilesystem,
        &target_root,
        std::slice::from_ref(&candidate),
    )
    .unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].missing_path, target_root.join("script.sh"));

    // Now create the file on disk under target_root; rerun; no finding.
    std::fs::write(target_root.join("script.sh"), b"#!/bin/sh\n").unwrap();
    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    assert!(
        findings.is_empty(),
        "existing file should not flag: {findings:?}"
    );
}

#[test]
fn preflight_handles_unicode_in_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    let settings = r#"{
        "hooks": {
            "PreToolUse": [
                { "hooks": [ { "type": "command", "command": "/tmp/café/script.sh" } ] }
            ]
        }
    }"#;
    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", settings.as_bytes());

    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].missing_path,
        PathBuf::from("/tmp/café/script.sh")
    );
}

#[test]
fn preflight_walks_real_claude_ctrl_hook_shape() {
    // Smoke test matching the actual claude-ctrl settings.json shape
    // (nested matcher + multiple hooks per event entry).
    let tmp = tempfile::tempdir().unwrap();
    let target_root = tmp.path().join("home");
    std::fs::create_dir_all(&target_root).unwrap();

    let settings = br#"{
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Write|Edit",
                    "hooks": [
                        { "type": "command", "command": "$HOME/.claude/hooks/test-gate.sh" },
                        { "type": "command", "command": "$HOME/.claude/hooks/pre-write.sh" }
                    ]
                },
                {
                    "matcher": "Bash",
                    "hooks": [
                        { "type": "command",
                          "command": "{ cat; echo; } >> $HOME/.claude/runtime/log.jsonl" },
                        { "type": "command", "command": "$HOME/.claude/hooks/pre-bash.sh" }
                    ]
                }
            ]
        },
        "statusLine": {
            "type": "command",
            "command": "$HOME/.claude/scripts/statusline.sh"
        }
    }"#;
    let source = tmp.path().join("ns/.claude/settings.json");
    let candidate = settings_candidate(&source, ".claude/settings.json", settings);

    let findings =
        preflight_settings_commands(&RealFilesystem, &target_root, &[candidate]).unwrap();
    // 3 real hook paths (test-gate, pre-write, pre-bash) + statusline; the
    // inline `{ cat; ... }` skips. All four missing under target_root.
    assert_eq!(findings.len(), 4, "got: {findings:#?}");
    let status_line_count = findings
        .iter()
        .filter(|f| matches!(f.kind, PreflightKind::StatusLine))
        .count();
    assert_eq!(status_line_count, 1);
}
