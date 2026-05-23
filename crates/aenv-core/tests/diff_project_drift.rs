//! Project drift detection.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::diff::project_drift;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::path::PathBuf;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn setup_active_project() -> (TempDir, TempDir, RegistryLayout, AdapterRegistry) {
    let aenv_home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let layout = RegistryLayout::new(aenv_home.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();

    // Namespace `solo` with one CLAUDE.md.
    write_file(
        &layout.manifest_path("solo"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&layout.namespace_dir("solo").join("CLAUDE.md"), "# Hello\n");

    // Pin and activate.
    write_file(&project.path().join(".aenv"), "solo\n");
    let leaf = NamespaceId::new("solo").unwrap();
    aenv_core::activate::activate_namespace(&fs, &layout, &adapters, project.path(), &leaf)
        .unwrap();

    (aenv_home, project, layout, adapters)
}

#[test]
fn no_drift_when_nothing_changed() {
    let (_aenv_home, project, layout, adapters) = setup_active_project();
    let fs = aenv_core::fs::RealFilesystem;
    let drift = project_drift(&fs, &layout, &adapters, project.path()).unwrap();
    assert!(drift.drifted.is_empty(), "got drift: {drift:?}");
}

#[test]
fn drift_when_symlink_replaced_with_edited_file() {
    let (_aenv_home, project, layout, adapters) = setup_active_project();
    let claude_path = project.path().join("CLAUDE.md");
    std::fs::remove_file(&claude_path).unwrap();
    std::fs::write(&claude_path, "# Hello\n\nLocal edit.\n").unwrap();
    let fs = aenv_core::fs::RealFilesystem;
    let drift = project_drift(&fs, &layout, &adapters, project.path()).unwrap();
    assert_eq!(drift.drifted.len(), 1);
    assert_eq!(drift.drifted[0].path, PathBuf::from("CLAUDE.md"));
    assert_eq!(drift.drifted[0].kind, "symlink-replaced");
}
