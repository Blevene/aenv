use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::PathBuf;

fn write(fs: &MockFilesystem, p: &str, b: &[u8]) {
    fs.write(&PathBuf::from(p), b).unwrap();
}

#[test]
fn authored_skill_materializes_at_project_path() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));

    // Adapter with skills_dir set.
    write(
        &fs,
        "/h/adapters/claude-code.toml",
        b"name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\nskills_dir = \".claude/skills\"\n",
    );

    // Namespace with one authored skill.
    write(
        &fs,
        "/h/envs/base/aenv.toml",
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n\n[[skills]]\nname = \"my-skill\"\nmode = \"authored\"\nadapter = \"claude-code\"\n",
    );
    write(&fs, "/h/envs/base/CLAUDE.md", b"hi");
    write(
        &fs,
        "/h/envs/base/.claude/skills/my-skill/SKILL.md",
        b"---\nname: my-skill\ndescription: y\n---\nbody\n",
    );

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
    let paths: Vec<_> = state.managed_files.iter().map(|m| m.path.clone()).collect();
    assert!(paths.contains(&PathBuf::from(".claude/skills/my-skill/SKILL.md")));
    assert!(fs
        .exists(&project.join(".claude/skills/my-skill/SKILL.md"))
        .unwrap());
}
