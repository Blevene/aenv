//! End-to-end composition tests against MockFilesystem.

use std::path::{Path, PathBuf};

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::MockFilesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::MaterializeStrategy;
use aenv_core::state::ActivationState;

const REG: &str = "/aenv";
const PROJ: &str = "/proj";

fn registry() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from(REG))
}

fn cc() -> Adapter {
    toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"#,
    )
    .unwrap()
}

fn mcp() -> Adapter {
    toml::from_str(
        r#"
name = "mcp"
files = [".mcp.json"]
[default_merge]
".mcp.json" = "deep"
"#,
    )
    .unwrap()
}

fn adapters() -> AdapterRegistry {
    let mut r = AdapterRegistry::default();
    r.insert(cc());
    r.insert(mcp());
    r
}

// MockFilesystem methods are &self with interior mutability.
fn write(fs: &MockFilesystem, p: &str, c: &str) {
    use aenv_core::fs::Filesystem;
    fs.write(Path::new(p), c.as_bytes()).unwrap();
}

fn read(fs: &MockFilesystem, p: &str) -> String {
    use aenv_core::fs::Filesystem;
    String::from_utf8(fs.read(Path::new(p)).unwrap()).unwrap()
}

#[test]
fn activates_two_namespace_chain_with_section_merge_and_symlinked_skill() {
    let fs = MockFilesystem::new();
    write(
        &fs,
        &format!("{REG}/envs/base/aenv.toml"),
        "name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write(
        &fs,
        &format!("{REG}/envs/base/CLAUDE.md"),
        "# Build & Test\n\ncargo test\n",
    );
    write(
        &fs,
        &format!("{REG}/envs/leaf/aenv.toml"),
        "name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write(
        &fs,
        &format!("{REG}/envs/leaf/CLAUDE.md"),
        "# Disposition\n\nbe terse\n",
    );

    activate_namespace(
        &fs,
        &registry(),
        &adapters(),
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap();

    let merged = read(&fs, &format!("{PROJ}/CLAUDE.md"));
    assert!(merged.contains("# Build & Test"));
    assert!(merged.contains("cargo test"));
    assert!(merged.contains("# Disposition"));
    assert!(merged.contains("be terse"));

    use aenv_core::fs::Filesystem;
    let meta = fs
        .symlink_metadata(Path::new(&format!("{PROJ}/CLAUDE.md")))
        .unwrap();
    assert!(!matches!(meta.kind, aenv_core::fs::FileKind::Symlink));

    let state: ActivationState = serde_json::from_slice(
        &fs.read(Path::new(&format!("{PROJ}/.aenv-state/state.json")))
            .unwrap(),
    )
    .unwrap();
    let claude = state
        .managed_files
        .iter()
        .find(|m| m.path.to_string_lossy().ends_with("CLAUDE.md"))
        .unwrap();
    assert!(matches!(claude.strategy, MaterializeStrategy::SectionMerge));
    assert_eq!(claude.contributors.len(), 2);
    assert!(claude.shadows.is_empty());
}

#[test]
fn deep_merges_mcp_json_across_chain() {
    let fs = MockFilesystem::new();
    write(
        &fs,
        &format!("{REG}/envs/base/aenv.toml"),
        "name = \"base\"\n[adapters.mcp]\nfiles = [\".mcp.json\"]\n",
    );
    write(
        &fs,
        &format!("{REG}/envs/base/.mcp.json"),
        r#"{"servers":{"a":{"command":"a"}}}"#,
    );
    write(
        &fs,
        &format!("{REG}/envs/leaf/aenv.toml"),
        "name = \"leaf\"\nextends = [\"base\"]\n[adapters.mcp]\nfiles = [\".mcp.json\"]\n",
    );
    write(
        &fs,
        &format!("{REG}/envs/leaf/.mcp.json"),
        r#"{"servers":{"b":{"command":"b"}}}"#,
    );

    activate_namespace(
        &fs,
        &registry(),
        &adapters(),
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap();

    let merged = read(&fs, &format!("{PROJ}/.mcp.json"));
    let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
    assert!(v["servers"]["a"]["command"] == "a");
    assert!(v["servers"]["b"]["command"] == "b");
}

#[test]
fn skill_overlay_shadows_parent_skill() {
    let cc_w_skills: Adapter = toml::from_str(
        "name = \"claude-code\"\nfiles = [\".claude/skills/write-tests/SKILL.md\"]\n",
    )
    .unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc_w_skills);

    let fs = MockFilesystem::new();
    write(&fs, &format!("{REG}/envs/base/aenv.toml"),
        "name = \"base\"\n[adapters.claude-code]\nfiles = [\".claude/skills/write-tests/SKILL.md\"]\n");
    write(
        &fs,
        &format!("{REG}/envs/base/.claude/skills/write-tests/SKILL.md"),
        "base impl",
    );
    write(&fs, &format!("{REG}/envs/leaf/aenv.toml"),
        "name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\".claude/skills/write-tests/SKILL.md\"]\n");
    write(
        &fs,
        &format!("{REG}/envs/leaf/.claude/skills/write-tests/SKILL.md"),
        "leaf impl",
    );

    activate_namespace(
        &fs,
        &registry(),
        &adapters,
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap();

    let body = read(&fs, &format!("{PROJ}/.claude/skills/write-tests/SKILL.md"));
    assert_eq!(body, "leaf impl");

    use aenv_core::fs::Filesystem;
    let state: ActivationState = serde_json::from_slice(
        &fs.read(Path::new(&format!("{PROJ}/.aenv-state/state.json")))
            .unwrap(),
    )
    .unwrap();
    let mf = state
        .managed_files
        .iter()
        .find(|m| m.path.to_string_lossy().contains("write-tests"))
        .unwrap();
    assert_eq!(mf.shadows.len(), 1);
    assert_eq!(mf.shadows[0].namespace().as_str(), "base");
}

#[test]
fn rollback_removes_prior_materialized_file_on_partial_failure() {
    // Two namespaces so both files have 2 candidates → both get merged as
    // regular files rather than symlinks. BTreeMap iteration order is
    // lexicographic: ".mcp.json" sorts before "CLAUDE.md", so .mcp.json is
    // written first. Injecting failure on CLAUDE.md means .mcp.json has
    // already been written before the failure fires — exercises
    // RemoveRegularFile undo.
    let fs = MockFilesystem::new();
    fs.fail_writes_to(Path::new(&format!("{PROJ}/CLAUDE.md")));
    // base namespace
    write(&fs, &format!("{REG}/envs/base/aenv.toml"),
        "name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n[adapters.mcp]\nfiles = [\".mcp.json\"]\n");
    write(&fs, &format!("{REG}/envs/base/CLAUDE.md"), "# base\n");
    write(
        &fs,
        &format!("{REG}/envs/base/.mcp.json"),
        r#"{"servers":{}}"#,
    );
    // leaf namespace extends base
    write(&fs, &format!("{REG}/envs/leaf/aenv.toml"),
        "name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n[adapters.mcp]\nfiles = [\".mcp.json\"]\n");
    write(&fs, &format!("{REG}/envs/leaf/CLAUDE.md"), "# leaf\n");
    write(
        &fs,
        &format!("{REG}/envs/leaf/.mcp.json"),
        r#"{"servers":{}}"#,
    );

    let err = activate_namespace(
        &fs,
        &registry(),
        &adapters(),
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap_err();
    assert!(
        matches!(err, aenv_core::AenvError::ActivationConflict(_))
            || matches!(err, aenv_core::AenvError::Io(_))
    );

    use aenv_core::fs::Filesystem;
    assert!(!fs.exists(Path::new(&format!("{PROJ}/.mcp.json"))).unwrap());
    assert!(!fs.exists(Path::new(&format!("{PROJ}/CLAUDE.md"))).unwrap());
    assert!(!fs
        .exists(Path::new(&format!("{PROJ}/.aenv-state/state.json")))
        .unwrap());
}
