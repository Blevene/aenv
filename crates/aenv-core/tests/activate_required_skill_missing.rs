use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::AenvError;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::PathBuf;

fn write(fs: &MockFilesystem, p: &str, b: &[u8]) {
    fs.write(&PathBuf::from(p), b).unwrap();
}

#[test]
fn required_unreachable_aborts_activation() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    write(
        &fs,
        "/h/adapters/claude-code.toml",
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\nskills_dir = \".claude/skills\"\n",
    );
    write(
        &fs,
        "/h/envs/base/aenv.toml",
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[[skills]]\nname = \"missing\"\nmode = \"imported\"\nadapter = \"claude-code\"\nsource = \"/does/not/exist\"\nrequired = true\n",
    );
    write(&fs, "/h/envs/base/CLAUDE.md", b"hi");

    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let err = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("base").unwrap(),
    )
    .unwrap_err();
    assert!(
        matches!(err, AenvError::ActivationConflict(_)),
        "expected ActivationConflict, got {err:?}"
    );
    assert_eq!(err.exit_code(), 13);
    // Project must be untouched (R-63).
    assert!(!fs.exists(&project.join(".aenv-state/state.json")).unwrap());
    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn unrequired_unreachable_warns_but_activates() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    write(
        &fs,
        "/h/adapters/claude-code.toml",
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\nskills_dir = \".claude/skills\"\n",
    );
    write(
        &fs,
        "/h/envs/base/aenv.toml",
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[[skills]]\nname = \"optional\"\nmode = \"imported\"\nadapter = \"claude-code\"\nsource = \"/does/not/exist\"\n",
    );
    write(&fs, "/h/envs/base/CLAUDE.md", b"hi");

    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    let project = PathBuf::from("/project");
    fs.create_dir_all(&project).unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &adapters,
        &project,
        &NamespaceId::new("base").unwrap(),
    )
    .expect("activation should succeed when optional skill is unreachable");
    assert_eq!(state.active_namespace, "base");
    // CLAUDE.md materialized; skill is absent.
    assert!(fs.exists(&project.join("CLAUDE.md")).unwrap());
    assert!(!fs
        .exists(&project.join(".claude/skills/optional/SKILL.md"))
        .unwrap());
}
