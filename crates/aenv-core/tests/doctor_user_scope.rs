//! Doctor evaluator awareness of user-scope candidates.
//!
//! The `instructions_max_chars` policy (and its R-24 auto-fire helper) must
//! consult `adapter.user_roles` + `adapter.user_soft_limits` for any candidate
//! with `Scope::User`, and label outcomes with a `~/`-prefixed path so users
//! see what they'll see once activation lands.

#[test]
fn doctor_reports_user_scope_soft_limit_violation() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    // Tiny user-scope soft limit; project side is unconstrained.
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
user_files = ["~/.claude/CLAUDE.md"]

[user_roles]
"~/.claude/CLAUDE.md" = "instructions"

[user_soft_limits]
instructions = 10
"#,
    )
    .unwrap();
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &adapters_dir).unwrap();

    let ns_dir = registry.namespace_dir("oversize");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), "x".repeat(500)).unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"
name = "oversize"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("oversize").unwrap();
    let resolution =
        aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution, &fake_home);

    let labels: Vec<String> = report
        .outcomes
        .iter()
        .map(|o| {
            let t = o
                .target
                .as_ref()
                .map_or(String::new(), std::string::ToString::to_string);
            format!("{} {}", o.key, t)
        })
        .collect();
    assert!(
        labels
            .iter()
            .any(|l| l.contains("instructions_max_chars") && l.contains("~/.claude/CLAUDE.md")),
        "no user-scope soft-limit violation reported: {labels:?}"
    );
}

#[test]
fn doctor_reports_unresolvable_hook_path() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().join("aenv"));
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(registry.adapters_dir()).unwrap();
    std::fs::write(
        registry.adapters_dir().join("claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/settings.json"]
"#,
    )
    .unwrap();
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &registry.adapters_dir()).unwrap();

    let ns_dir = registry.namespace_dir("hooky");
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(
        ns_dir.join("user/.claude/settings.json"),
        br#"{
            "hooks": {
                "PreToolUse": [
                    { "hooks": [ { "type": "command",
                        "command": "/definitely/nope/policy.py" } ] }
                ]
            }
        }"#,
    )
    .unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "hooky"
[adapters.claude-code]
user_files = [".claude/settings.json"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("hooky").unwrap();
    let resolution =
        aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution, &fake_home);

    let hits: Vec<&aenv_core::policies::builtin::PolicyOutcome> = report
        .outcomes
        .iter()
        .filter(|o| o.key == "hook_paths_resolvable")
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected exactly one hook_paths_resolvable outcome, got: {hits:?}"
    );
    let only = hits[0];
    assert!(
        matches!(
            only.status,
            aenv_core::policies::builtin::OutcomeStatus::Warn { .. }
        ),
        "preflight outcome must be Warn, not Fail/Pass: {only:?}"
    );
    if let aenv_core::policies::builtin::OutcomeStatus::Warn { msg } = &only.status {
        assert!(msg.contains("/definitely/nope/policy.py"), "msg: {msg}");
        assert!(msg.contains("PreToolUse"), "msg: {msg}");
    }
    let target_str = only
        .target
        .as_ref()
        .map(std::string::ToString::to_string)
        .unwrap_or_default();
    assert!(
        target_str.contains("settings.json"),
        "target should mention settings.json: {target_str}"
    );
}

#[test]
fn doctor_silent_when_all_hook_paths_resolve() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = aenv_core::home::RegistryLayout::new(tmp.path().join("aenv"));
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(registry.adapters_dir()).unwrap();
    std::fs::write(
        registry.adapters_dir().join("claude-code.toml"),
        r#"name = "claude-code"
user_files = ["~/.claude/settings.json", "~/.claude/hooks/h.sh"]
"#,
    )
    .unwrap();
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(&fs, &registry.adapters_dir()).unwrap();

    let ns_dir = registry.namespace_dir("clean");
    std::fs::create_dir_all(ns_dir.join("user/.claude/hooks")).unwrap();
    std::fs::write(
        ns_dir.join("user/.claude/settings.json"),
        br#"{
            "hooks": {
                "PreToolUse": [
                    { "hooks": [ { "type": "command",
                        "command": "$HOME/.claude/hooks/h.sh" } ] }
                ]
            }
        }"#,
    )
    .unwrap();
    std::fs::write(ns_dir.join("user/.claude/hooks/h.sh"), b"#!/bin/sh\n").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        r#"name = "clean"
[adapters.claude-code]
user_files = [".claude/settings.json", ".claude/hooks/h.sh"]
"#,
    )
    .unwrap();

    let leaf = aenv_core::identity::NamespaceId::new("clean").unwrap();
    let resolution =
        aenv_core::resolve::resolve_namespace(&fs, &registry, &adapters, &leaf).unwrap();
    let fake_home = tmp.path().join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let report = aenv_core::doctor::evaluate(&fs, &registry, &adapters, &resolution, &fake_home);

    let hits: Vec<&aenv_core::policies::builtin::PolicyOutcome> = report
        .outcomes
        .iter()
        .filter(|o| o.key == "hook_paths_resolvable")
        .collect();
    assert!(
        hits.is_empty(),
        "expected no hook_paths_resolvable outcomes (path is materialized this run), got: {hits:?}"
    );
}
