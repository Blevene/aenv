use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::{Path, PathBuf};

fn write(fs: &MockFilesystem, p: &str, b: &[u8]) {
    fs.write(&PathBuf::from(p), b).unwrap();
}

#[test]
fn imported_local_skill_materializes_from_source() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    write(
        &fs,
        "/h/adapters/claude-code.toml",
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\nskills_dir = \".claude/skills\"\n",
    );

    // External skill source
    write(
        &fs,
        "/external/my-import/SKILL.md",
        b"---\nname: my-import\ndescription: yo\n---\nbody\n",
    );

    // Namespace declares an imported skill from a local path.
    write(
        &fs,
        "/h/envs/base/aenv.toml",
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[[skills]]\nname = \"my-import\"\nmode = \"imported\"\nadapter = \"claude-code\"\nsource = \"/external/my-import\"\n",
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
    .unwrap();
    let imported = state
        .managed_files
        .iter()
        .find(|m| m.path == Path::new(".claude/skills/my-import/SKILL.md"))
        .expect("imported skill should appear in managed files");
    assert!(
        imported.skill_provenance.is_some(),
        "expected skill_provenance on imported file"
    );
    let prov = imported.skill_provenance.as_ref().unwrap();
    assert_eq!(prov.source, "/external/my-import");
    assert!(prov.resolved_hash.starts_with("sha256:"));
}
