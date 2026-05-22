//! End-to-end integration tests reproducing functional spec §5.5 (parameter
//! queries) and §5.12 (doctor clean + violation). Uses the raw
//! `std::process::Command` + `Harness` pattern (no assert_cmd).

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

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

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn write_file(dir: &Path, rel: &str, body: &[u8]) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

// ---------------------------------------------------------------------------
// Test 1: provenance walk matches spec §5.5
// ---------------------------------------------------------------------------

/// Reproduce the spec §5.5 example chain (base → detailed-execution) and
/// verify the parameter-query examples.
#[test]
fn provenance_walk_matches_spec_5_5() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");
    let out = h
        .cmd()
        .args(["create", "detailed-execution"])
        .output()
        .unwrap();
    assert_success(&out, "create detailed-execution");

    let home = h.aenv_home();

    std::fs::write(
        home.join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-sonnet-4.6"
instructions_budget = 5000

[policies]
skill_requires_description = true
"#,
    )
    .unwrap();
    write_file(&home.join("envs/base"), "CLAUDE.md", b"base body");

    std::fs::write(
        home.join("envs/detailed-execution/aenv.toml"),
        r#"
name = "detailed-execution"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md"]

[parameters]
default_model = "claude-opus-4.7"
instructions_budget = 3000
"#,
    )
    .unwrap();
    write_file(
        &home.join("envs/detailed-execution"),
        "CLAUDE.md",
        b"leaf body",
    );

    // Explicit-namespace get: spec example `aenv get detailed-execution.default_model`
    // → opus + overrides base + prior value from base.
    let out = h
        .cmd()
        .args(["get", "detailed-execution.default_model"])
        .output()
        .unwrap();
    assert_success(&out, "get detailed-execution.default_model");
    let s = stdout(&out);
    assert!(
        s.contains("claude-opus-4.7"),
        "expected 'claude-opus-4.7' in output; got: {s:?}"
    );
    assert!(
        s.contains("source: detailed-execution"),
        "expected 'source: detailed-execution'; got: {s:?}"
    );
    assert!(
        s.contains("overrides base"),
        "expected 'overrides base' in provenance; got: {s:?}"
    );
    assert!(
        s.contains("claude-sonnet-4.6"),
        "expected prior value 'claude-sonnet-4.6'; got: {s:?}"
    );

    // `aenv get detailed-execution.instructions_budget` → 3000, overrides base (5000).
    let out = h
        .cmd()
        .args(["get", "detailed-execution.instructions_budget"])
        .output()
        .unwrap();
    assert_success(&out, "get detailed-execution.instructions_budget");
    let s = stdout(&out);
    assert!(s.contains("3000"), "expected '3000' in output; got: {s:?}");
    assert!(
        s.contains("source: detailed-execution"),
        "expected 'source: detailed-execution'; got: {s:?}"
    );
    assert!(
        s.contains("overrides base"),
        "expected 'overrides base' in provenance; got: {s:?}"
    );
    assert!(s.contains("5000"), "expected '5000' in output; got: {s:?}");

    // `skill_requires_description` is a *policy*, not a parameter — `get` must
    // exit 16 (ParameterUndefined).
    let out = h
        .cmd()
        .args(["get", "detailed-execution.skill_requires_description"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "expected failure for policy key via get"
    );
    assert_eq!(
        out.status.code(),
        Some(16),
        "expected exit 16 (ParameterUndefined) for policy key; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // `aenv doctor detailed-execution` should run cleanly (CLAUDE.md is short;
    // no authored skills in the namespace) and list the active policy.
    let out = h
        .cmd()
        .args(["doctor", "detailed-execution"])
        .output()
        .unwrap();
    assert_success(&out, "doctor detailed-execution");
    let s = stdout(&out);
    assert!(
        s.contains("Active policies"),
        "expected 'Active policies' in doctor output; got: {s:?}"
    );
    assert!(
        s.contains("skill_requires_description"),
        "expected 'skill_requires_description' in doctor output; got: {s:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: doctor violation matches spec §5.12
// ---------------------------------------------------------------------------

/// Reproduce spec §5.12 violation example: CLAUDE.md too long + missing skill
/// description. Both policies are advisory so doctor exits 0 but reports POLICY.
#[test]
fn doctor_violation_matches_spec_5_12() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "base"]).output().unwrap();
    assert_success(&out, "create base");
    let out = h
        .cmd()
        .args(["create", "experiments-overgrown"])
        .output()
        .unwrap();
    assert_success(&out, "create experiments-overgrown");

    let home = h.aenv_home();

    std::fs::write(
        home.join("envs/base/aenv.toml"),
        r#"
name = "base"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = 5000
skill_requires_description = true
"#,
    )
    .unwrap();
    write_file(&home.join("envs/base"), "CLAUDE.md", b"ok");

    // Build an 8247-char CLAUDE.md and a half-baked-skill with no description.
    let big = "x".repeat(8247);
    std::fs::write(
        home.join("envs/experiments-overgrown/aenv.toml"),
        r#"
name = "experiments-overgrown"
extends = ["base"]

[adapters.claude-code]
files = ["CLAUDE.md", ".claude/skills/half-baked-skill/SKILL.md"]
"#,
    )
    .unwrap();
    write_file(
        &home.join("envs/experiments-overgrown"),
        "CLAUDE.md",
        big.as_bytes(),
    );
    write_file(
        &home.join("envs/experiments-overgrown"),
        ".claude/skills/half-baked-skill/SKILL.md",
        b"---\nname: half-baked-skill\n---\nNo description.\n",
    );

    // Doctor should exit 0 (advisory) but report both violations.
    let out = h
        .cmd()
        .args(["doctor", "experiments-overgrown"])
        .output()
        .unwrap();
    let s = stdout(&out);
    assert!(
        out.status.success(),
        "expected exit 0 (advisory policies only); status={:?}, stdout={s}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        s.contains("POLICY"),
        "expected 'POLICY' in doctor output; got: {s:?}"
    );
    assert!(
        s.contains("instructions_max_chars"),
        "expected 'instructions_max_chars'; got: {s:?}"
    );
    assert!(
        s.contains("8247"),
        "expected '8247' (actual char count) in output; got: {s:?}"
    );
    assert!(
        s.contains("5000"),
        "expected '5000' (limit) in output; got: {s:?}"
    );
    assert!(
        s.contains("skill_requires_description"),
        "expected 'skill_requires_description'; got: {s:?}"
    );
    assert!(
        s.contains("half-baked-skill"),
        "expected 'half-baked-skill' in output; got: {s:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: activate refused when enforce violation
// ---------------------------------------------------------------------------

/// When `instructions_max_chars` is `enforce = true` and the CLAUDE.md
/// exceeds the limit, `aenv activate` must exit 17 and leave the project
/// untouched (no state file, no CLAUDE.md).
#[test]
fn activate_refused_when_enforce_violation() {
    let h = Harness::new();

    let out = h.cmd().args(["create", "tight"]).output().unwrap();
    assert_success(&out, "create tight");

    let home = h.aenv_home();
    let big = "x".repeat(8000);
    std::fs::write(
        home.join("envs/tight/aenv.toml"),
        r#"
name = "tight"

[adapters.claude-code]
files = ["CLAUDE.md"]

[policies]
instructions_max_chars = { value = 5000, enforce = true }
"#,
    )
    .unwrap();
    write_file(&home.join("envs/tight"), "CLAUDE.md", big.as_bytes());

    // Pin the project to the namespace.
    let out = h
        .cmd()
        .args(["use", "tight", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_success(&out, "use tight");

    // Activate must fail with exit 17 (PolicyViolation / enforce).
    let out = h
        .cmd()
        .args(["activate", "--project"])
        .arg(h.project())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(17),
        "expected exit 17 (PolicyViolation); status={:?}, stdout={}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Project must remain completely untouched (R-63).
    assert!(
        !h.project().join(".aenv-state/state.json").exists(),
        "state.json must not exist after blocked activation"
    );
    assert!(
        !h.project().join("CLAUDE.md").exists(),
        "CLAUDE.md must not be materialized after blocked activation"
    );
}
