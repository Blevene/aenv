//! Tests for the activation primitive `activate_namespace`.
//!
//! Mock-driven so we can exercise rollback paths via fail injection.
//! Real-filesystem end-to-end coverage lives in `aenv-cli/tests/cli_e2e.rs`.

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::namespace::create_namespace;
use aenv_core::state::{ActivationState, MaterializeStrategy};
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

fn claude_adapter() -> Adapter {
    Adapter {
        name: "claude-code".to_string(),
        files: vec!["CLAUDE.md".to_string()],
        merge_strategies: Default::default(),
        roles: Default::default(),
        default_merge: Default::default(),
        parameters: vec![],
        skills_dir: None,
    }
}

fn setup_registry_with_namespace(fs: &MockFilesystem, ns: &str, files: &[(&str, &[u8])]) {
    let layout = layout();
    create_namespace(fs, &layout, ns).unwrap();
    // Patch the manifest to reference claude-code so the adapter's files apply.
    let manifest = format!("name = \"{ns}\"\n\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n");
    fs.write(&layout.manifest_path(ns), manifest.as_bytes())
        .unwrap();
    for (rel, content) in files {
        fs.write(&layout.namespace_dir(ns).join(rel), content)
            .unwrap();
    }
}

fn registry_with_claude() -> AdapterRegistry {
    let mut r = AdapterRegistry::new();
    r.insert(claude_adapter());
    r
}

#[test]
fn symlinks_new_file_into_project() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"disposition")]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    // Project file is a symlink to the namespace file.
    assert!(fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
    assert_eq!(
        fs.read_link(&project.join("CLAUDE.md")).unwrap(),
        layout.namespace_dir("experiments").join("CLAUDE.md")
    );

    // State records exactly that.
    assert_eq!(state.active_namespace, "experiments");
    assert_eq!(state.managed_files.len(), 1);
    assert_eq!(state.managed_files[0].path, PathBuf::from("CLAUDE.md"));
    assert_eq!(
        state.managed_files[0].strategy,
        MaterializeStrategy::Symlink
    );
    assert!(state.backed_up.is_empty());

    // State file is persisted at .aenv-state/state.json.
    let on_disk = fs.read(&project.join(".aenv-state/state.json")).unwrap();
    let parsed = ActivationState::from_json(&String::from_utf8(on_disk).unwrap()).unwrap();
    assert_eq!(parsed, state);
}

#[test]
fn errors_when_namespace_does_not_exist() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    let err = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("missing").unwrap(),
    )
    .expect_err("must error");
    assert!(matches!(err, aenv_core::AenvError::NamespaceNotFound(_)));
    assert_eq!(err.exit_code(), 10);
}

#[test]
fn errors_when_manifest_names_unknown_adapter() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments").unwrap();
    // Manifest names an adapter not in the registry.
    let manifest = "name = \"experiments\"\n\n[adapters.cursor]\nfiles = [\".cursorrules\"]\n";
    fs.write(&layout.manifest_path("experiments"), manifest.as_bytes())
        .unwrap();
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();

    let err = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .expect_err("must error");
    assert!(matches!(err, aenv_core::AenvError::AdapterMissing(_)));
    assert_eq!(err.exit_code(), 11);
}

#[test]
fn missing_adapter_file_is_skipped_silently() {
    // If the adapter declares CLAUDE.md but the namespace doesn't ship it,
    // nothing happens for that file.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();
    assert!(state.managed_files.is_empty());
}

#[test]
fn backs_up_displaced_project_file() {
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(
        &fs,
        "experiments",
        &[("CLAUDE.md", b"namespace disposition")],
    );
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), b"user-authored")
        .unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    // Project file is now a symlink.
    assert!(fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
    // Backup file holds the original contents.
    assert_eq!(state.backed_up.len(), 1);
    let backup = &state.backed_up[0];
    assert_eq!(backup.original_path, PathBuf::from("CLAUDE.md"));
    let backed_bytes = fs.read(&project.join(&backup.backup_path)).unwrap();
    assert_eq!(backed_bytes, b"user-authored");
}

#[test]
fn byte_identical_file_is_managed_in_place_not_symlinked() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let body: &[u8] = b"# CLAUDE.md\nshared content\n";
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", body)]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), body).unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    // R-46: file matches namespace -> leave in place, do NOT symlink, mark managed.
    assert!(!fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
    assert_eq!(state.managed_files.len(), 1);
    assert_eq!(
        state.managed_files[0].strategy,
        MaterializeStrategy::Identical
    );
    assert!(state.backed_up.is_empty(), "no backup needed");
}

#[test]
fn aenv_managed_symlink_pointing_at_same_target_is_left_alone() {
    // Edge case: previous activation left a symlink pointing exactly where
    // we'd point now. No-op rather than backup + recreate.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"x")]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    // Pre-existing symlink to the same target.
    fs.symlink(
        &layout.namespace_dir("experiments").join("CLAUDE.md"),
        &project.join("CLAUDE.md"),
    )
    .unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    // No backup; symlink stays.
    assert!(state.backed_up.is_empty());
    assert!(fs.is_symlink(&project.join("CLAUDE.md")).unwrap());
}

#[test]
fn stale_symlink_to_other_target_is_displaced() {
    // Regression: a project path that's a symlink to a non-aenv target (or
    // a stale aenv symlink whose target is gone) was previously
    // misclassified as Absent — exists() follows symlinks. The fix checks
    // symlink_metadata BEFORE exists, so the link itself surfaces as
    // Displaced and gets backed up rather than overwritten silently.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"new")]);
    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    // Pre-existing symlink to a path that does NOT exist (stale).
    fs.symlink(
        &PathBuf::from("/elsewhere/CLAUDE.md"),
        &project.join("CLAUDE.md"),
    )
    .unwrap();

    let state = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .unwrap();

    // Backed up the stale link; fresh symlink in place pointing at our source.
    assert_eq!(state.backed_up.len(), 1);
    assert_eq!(state.backed_up[0].original_path, PathBuf::from("CLAUDE.md"));
    assert_eq!(
        fs.read_link(&project.join("CLAUDE.md")).unwrap(),
        layout.namespace_dir("experiments").join("CLAUDE.md")
    );
}

#[test]
fn rolls_back_when_state_write_fails_after_displacement() {
    // Setup: a project with a user-authored CLAUDE.md (which will be
    // displaced to backup during activation), then inject a write failure
    // on .aenv-state/state.json — the final write in perform_activation. The
    // failure fires after both the backup-rename and the symlink-create
    // have succeeded, so rollback must replay BOTH undo steps in reverse:
    // remove the new symlink, then rename the backup back into place.
    let fs = MockFilesystem::new();
    let layout = layout();
    setup_registry_with_namespace(&fs, "experiments", &[("CLAUDE.md", b"namespace")]);

    let project = PathBuf::from("/projects/p");
    fs.create_dir_all(&project).unwrap();
    fs.write(&project.join("CLAUDE.md"), b"user-authored")
        .unwrap();
    fs.fail_writes_to(&project.join(".aenv-state/state.json"));

    let err = activate_namespace(
        &fs,
        &layout,
        &registry_with_claude(),
        &project,
        &NamespaceId::new("experiments").unwrap(),
    )
    .expect_err("must error");
    assert!(matches!(
        err,
        aenv_core::AenvError::Io(_) | aenv_core::AenvError::ActivationConflict(_)
    ));

    // Rollback invariants:
    // 1. No symlink left at the project path.
    assert!(
        !fs.is_symlink(&project.join("CLAUDE.md")).unwrap_or(false),
        "symlink should be rolled back"
    );
    // 2. Original content restored.
    assert_eq!(
        fs.read(&project.join("CLAUDE.md")).unwrap(),
        b"user-authored"
    );
    // 3. No state.json on disk.
    assert!(!fs.exists(&project.join(".aenv-state/state.json")).unwrap());
}
