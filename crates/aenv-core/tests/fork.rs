use std::path::{Path, PathBuf};

use aenv_core::activate::fork_file;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::state::ActivationState;

const REG: &str = "/aenv";
const PROJ: &str = "/proj";

fn registry() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from(REG))
}

fn setup_activated_chain(fs: &MockFilesystem) {
    let cc: Adapter = toml::from_str(
        r#"
name = "claude-code"
files = [".claude/skills/X/SKILL.md", "CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"#,
    )
    .unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc);

    fs.write(
        Path::new(&format!("{REG}/envs/leaf/aenv.toml")),
        b"name = \"leaf\"\n[adapters.claude-code]\nfiles = [\".claude/skills/X/SKILL.md\", \"CLAUDE.md\"]\n",
    )
    .unwrap();
    fs.write(
        Path::new(&format!("{REG}/envs/leaf/.claude/skills/X/SKILL.md")),
        b"the skill body",
    )
    .unwrap();
    fs.write(
        Path::new(&format!("{REG}/envs/leaf/CLAUDE.md")),
        b"# leaf\n",
    )
    .unwrap();

    aenv_core::activate::activate_namespace(
        fs,
        &registry(),
        &adapters,
        Path::new(PROJ),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap();
}

#[test]
fn forking_a_symlink_replaces_it_with_a_regular_file_with_same_bytes() {
    let fs = MockFilesystem::new();
    setup_activated_chain(&fs);

    let skill_str = format!("{PROJ}/.claude/skills/X/SKILL.md");
    let skill = Path::new(&skill_str);
    assert!(matches!(
        fs.symlink_metadata(skill).unwrap().kind,
        aenv_core::fs::FileKind::Symlink
    ));

    fork_file(&fs, Path::new(PROJ), Path::new(".claude/skills/X/SKILL.md")).unwrap();

    assert!(!matches!(
        fs.symlink_metadata(skill).unwrap().kind,
        aenv_core::fs::FileKind::Symlink
    ));
    assert_eq!(fs.read(skill).unwrap(), b"the skill body");

    let state_json_str = format!("{PROJ}/.aenv-state/state.json");
    let state_body = fs.read(Path::new(&state_json_str)).unwrap();
    let state: ActivationState = aenv_core::state::ActivationState::from_json(
        std::str::from_utf8(&state_body).unwrap(),
    )
    .unwrap();
    assert!(state
        .managed_files
        .iter()
        .all(|m| !m.path.to_string_lossy().contains("SKILL.md")));
}

#[test]
fn forking_an_unmanaged_path_errors() {
    let fs = MockFilesystem::new();
    setup_activated_chain(&fs);
    let err = fork_file(&fs, Path::new(PROJ), Path::new("other.txt")).unwrap_err();
    assert!(err.to_string().contains("not managed"));
}
