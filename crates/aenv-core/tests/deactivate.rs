//! Tests for deactivate_namespace.

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::deactivate::deactivate_namespace;
use aenv_core::fs::{Filesystem, MockFilesystem, RealFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::namespace::create_namespace;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

fn registry_with_claude() -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    r.insert(Adapter {
        name: "claude-code".to_string(),
        files: vec!["CLAUDE.md".to_string()],
        merge_strategies: Default::default(),
        roles: Default::default(),
        default_merge: Default::default(),
        parameters: vec![],
        skills_dir: None,
        soft_limits: Default::default(),
        user_files: Default::default(),
        user_roles: Default::default(),
        user_default_merge: Default::default(),
        user_merge_strategies: Default::default(),
        user_soft_limits: Default::default(),
        user_skills_dir: None,
        materialize: None,
    });
    r
}

fn setup_namespace(fs: &MockFilesystem, ns: &str, body: &[u8]) {
    let layout = layout();
    create_namespace(fs, &layout, ns, &[], &[]).unwrap();
    fs.write(
        &layout.manifest_path(ns),
        format!("name = \"{ns}\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n").as_bytes(),
    )
    .unwrap();
    fs.write(&layout.namespace_dir(ns).join("CLAUDE.md"), body)
        .unwrap();
}

#[test]
fn deactivate_removes_symlink_and_state() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"disposition");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
    assert!(!fs.exists(&project.join(".aenv-state/state.json")).unwrap());
}

#[test]
fn deactivate_restores_backed_up_originals() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"namespace");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), b"original").unwrap();
    activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    let restored = fs.read(&project.join("CLAUDE.md")).unwrap();
    assert_eq!(restored, b"original");
    assert!(!fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn deactivate_leaves_unmanaged_files_alone() {
    // R-48: aenv removes only files it materialized.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_namespace(&fs, "experiments", b"x");
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    // User creates a file during activation.
    fs.write(&project.join("README.md"), b"user file").unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    // Symlink removed; user file untouched.
    assert!(!fs.exists(&project.join("CLAUDE.md")).unwrap());
    assert_eq!(fs.read(&project.join("README.md")).unwrap(), b"user file");
}

#[test]
fn deactivate_errors_when_no_state_file() {
    // "No active state to deactivate" is distinct from "no .aenv pin":
    // a user can have a pin (project is associated with a namespace) but
    // have never activated. ActivationConflict (exit 13) is the right
    // variant; ProjectNotPinned (exit 20) is specifically about the
    // pin file itself missing.
    let fs = MockFilesystem::new();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = deactivate_namespace(&fs, &project).expect_err("must error");
    assert!(matches!(err, aenv_core::AenvError::ActivationConflict(_)));
    assert_eq!(err.exit_code(), 13);
}

#[test]
fn deactivate_leaves_identical_file_in_place() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let body: &[u8] = b"shared";
    setup_namespace(&fs, "experiments", body);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), body).unwrap(); // identical
    activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    deactivate_namespace(&fs, &project).unwrap();

    // Identical-strategy file is the user's; it stays.
    assert_eq!(fs.read(&project.join("CLAUDE.md")).unwrap(), body);
}

#[test]
fn deactivate_removes_empty_state_directory() {
    // After deactivation the .aenv-state/ directory should be removed if empty.
    // This test uses RealFilesystem + tempdir because std::fs::remove_dir is called
    // directly in deactivate_namespace (the Filesystem trait has no remove_dir).
    let aenv_home_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(aenv_home_dir.path()).unwrap();
    let project = std::fs::canonicalize(project_dir.path()).unwrap();

    let layout = RegistryLayout::new(aenv_home);
    let fs = RealFilesystem;

    // Build a real namespace with a CLAUDE.md artifact.
    let ns_dir = layout.namespace_dir("base");
    std::fs::create_dir_all(&ns_dir).unwrap();
    std::fs::write(
        layout.manifest_path("base"),
        b"name = \"base\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    )
    .unwrap();
    std::fs::write(ns_dir.join("CLAUDE.md"), b"content").unwrap();

    let mut reg = AdapterRegistry::new();
    reg.insert(Adapter {
        name: "claude-code".to_string(),
        files: vec!["CLAUDE.md".to_string()],
        merge_strategies: Default::default(),
        roles: Default::default(),
        default_merge: Default::default(),
        parameters: vec![],
        skills_dir: None,
        soft_limits: Default::default(),
        user_files: Default::default(),
        user_roles: Default::default(),
        user_default_merge: Default::default(),
        user_merge_strategies: Default::default(),
        user_soft_limits: Default::default(),
        user_skills_dir: None,
        materialize: None,
    });

    activate_namespace(
        &fs,
        &layout,
        &reg,
        &project,
        &NamespaceId::new("base").unwrap(),
    )
    .unwrap();

    let state_dir = project.join(".aenv-state");
    assert!(
        state_dir.exists(),
        ".aenv-state/ should exist after activate"
    );

    deactivate_namespace(&fs, &project).unwrap();

    assert!(
        !state_dir.exists(),
        ".aenv-state/ should be removed after deactivate when empty"
    );
}

#[test]
fn deactivate_prunes_empty_parent_directories() {
    // Regression: a namespace whose managed files sit several levels deep
    // (e.g. `.claude/skills/<skill>/references/<file>`) used to leave empty
    // parent dirs behind after deactivate. Pruning is best-effort, so user
    // files at the same level are preserved.
    let aenv_home_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();
    let aenv_home = std::fs::canonicalize(aenv_home_dir.path()).unwrap();
    let project = std::fs::canonicalize(project_dir.path()).unwrap();

    let layout = RegistryLayout::new(aenv_home);
    let fs = RealFilesystem;

    // Namespace declares the claude-code adapter and a deeply-nested file
    // (mirroring the layout an imported skill materializes into).
    let ns_dir = layout.namespace_dir("deep");
    std::fs::create_dir_all(&ns_dir).unwrap();
    std::fs::write(
        layout.manifest_path("deep"),
        b"name = \"deep\"\n\n[adapters.claude-code]\nfiles = [\".claude/skills/scanpy/references/api.md\"]\n",
    )
    .unwrap();
    let src_dir = ns_dir.join(".claude/skills/scanpy/references");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("api.md"), b"scanpy api").unwrap();

    let mut reg = AdapterRegistry::new();
    reg.insert(Adapter {
        name: "claude-code".to_string(),
        files: vec![".claude/skills/scanpy/references/api.md".to_string()],
        merge_strategies: Default::default(),
        roles: Default::default(),
        default_merge: Default::default(),
        parameters: vec![],
        skills_dir: None,
        soft_limits: Default::default(),
        user_files: Default::default(),
        user_roles: Default::default(),
        user_default_merge: Default::default(),
        user_merge_strategies: Default::default(),
        user_soft_limits: Default::default(),
        user_skills_dir: None,
        materialize: None,
    });

    // A USER file sitting next to the managed tree must survive deactivate.
    let user_file = project.join(".claude/notes.md");
    std::fs::create_dir_all(user_file.parent().unwrap()).unwrap();
    std::fs::write(&user_file, b"my notes").unwrap();

    activate_namespace(
        &fs,
        &layout,
        &reg,
        &project,
        &NamespaceId::new("deep").unwrap(),
    )
    .unwrap();
    assert!(project
        .join(".claude/skills/scanpy/references/api.md")
        .exists());

    deactivate_namespace(&fs, &project).unwrap();

    // The empty intermediate dirs that ONLY held managed files should be pruned.
    assert!(
        !project.join(".claude/skills/scanpy/references").exists(),
        ".claude/skills/scanpy/references/ should be pruned"
    );
    assert!(
        !project.join(".claude/skills/scanpy").exists(),
        ".claude/skills/scanpy/ should be pruned"
    );
    assert!(
        !project.join(".claude/skills").exists(),
        ".claude/skills/ should be pruned"
    );
    // BUT .claude/ contained a user file (notes.md), so it must stay.
    assert!(
        project.join(".claude").exists(),
        ".claude/ must survive — it still holds notes.md"
    );
    assert!(
        user_file.exists(),
        "user file .claude/notes.md must be untouched"
    );
}
