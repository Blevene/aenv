//! Regression guard for the interim between Milestones B and C.
//!
//! After Milestone B, the resolver emits candidates for both scopes. Until
//! Milestone C lands a scope-aware activation entry point, the existing
//! project-only callers (`activate_namespace`, `compute_material_set`) must
//! filter user-scope candidates out themselves — otherwise a namespace that
//! declares `user_files` would (a) materialize them into the project root
//! and (b) fold them into the project-scope hash.

use std::path::PathBuf;

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::RealFilesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::compute_material_set;

fn setup_registry(tmp: &std::path::Path) -> RegistryLayout {
    let registry = RegistryLayout::new(tmp.to_path_buf());
    let adapters_dir = registry.adapters_dir();
    std::fs::create_dir_all(&adapters_dir).unwrap();
    std::fs::write(
        adapters_dir.join("claude-code.toml"),
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
user_files = ["~/.claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    registry
}

fn write_namespace_with_both_scopes(registry: &RegistryLayout, name: &str) -> PathBuf {
    let ns_dir = registry.namespace_dir(name);
    std::fs::create_dir_all(ns_dir.join("user/.claude")).unwrap();
    std::fs::write(ns_dir.join("CLAUDE.md"), b"project body").unwrap();
    std::fs::write(ns_dir.join("user/.claude/CLAUDE.md"), b"user body").unwrap();
    std::fs::write(
        ns_dir.join("aenv.toml"),
        format!(
            r#"
name = "{name}"
[adapters.claude-code]
files = ["CLAUDE.md"]
user_files = [".claude/CLAUDE.md"]
"#
        ),
    )
    .unwrap();
    ns_dir
}

#[test]
fn activate_namespace_ignores_user_scope_candidates() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = setup_registry(tmp.path());
    let project_root = tmp.path().join("project");
    std::fs::create_dir_all(&project_root).unwrap();

    write_namespace_with_both_scopes(&registry, "foo");
    let fs = RealFilesystem;
    let adapters =
        AdapterRegistry::load_from_dir(&fs, &registry.adapters_dir()).unwrap();
    let leaf = NamespaceId::new("foo").unwrap();
    let state =
        activate_namespace(&fs, &registry, &adapters, &project_root, &leaf).unwrap();

    // Only the project CLAUDE.md should be materialized.
    let managed_paths: Vec<String> = state
        .managed_files
        .iter()
        .map(|m| m.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(managed_paths, vec!["CLAUDE.md".to_string()]);
    let claude = project_root.join("CLAUDE.md");
    assert!(claude.exists(), "CLAUDE.md not at project root: {claude:?}");
    assert_eq!(std::fs::read(&claude).unwrap(), b"project body");

    // No file should have landed under the user-scope path in the project root.
    let stray = project_root.join(".claude/CLAUDE.md");
    assert!(
        !stray.exists(),
        "user-scope file leaked into project root: {stray:?}"
    );
}

#[test]
fn project_hash_is_stable_when_user_files_are_added() {
    let fs = RealFilesystem;

    // Namespace A: project file only.
    let tmp_a = tempfile::tempdir().unwrap();
    let reg_a = setup_registry(tmp_a.path());
    let ns_a = reg_a.namespace_dir("only-project");
    std::fs::create_dir_all(&ns_a).unwrap();
    std::fs::write(ns_a.join("CLAUDE.md"), b"project body").unwrap();
    std::fs::write(
        ns_a.join("aenv.toml"),
        r#"
name = "only-project"
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    )
    .unwrap();
    let adapters_a =
        AdapterRegistry::load_from_dir(&fs, &reg_a.adapters_dir()).unwrap();
    let ms_a = compute_material_set(
        &fs,
        &reg_a,
        &adapters_a,
        &NamespaceId::new("only-project").unwrap(),
    )
    .unwrap();
    let hash_a = aenv_core::hash::hash_resolved_namespace(&ms_a);

    // Namespace B: same project content + a user_files declaration that should
    // be invisible to the project-scope hash.
    let tmp_b = tempfile::tempdir().unwrap();
    let reg_b = setup_registry(tmp_b.path());
    // Reuse the same namespace name for path-stability in the hash.
    let ns_b = reg_b.namespace_dir("only-project");
    std::fs::create_dir_all(ns_b.join("user/.claude")).unwrap();
    std::fs::write(ns_b.join("CLAUDE.md"), b"project body").unwrap();
    std::fs::write(ns_b.join("user/.claude/CLAUDE.md"), b"different user body").unwrap();
    std::fs::write(
        ns_b.join("aenv.toml"),
        r#"
name = "only-project"
[adapters.claude-code]
files = ["CLAUDE.md"]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let adapters_b =
        AdapterRegistry::load_from_dir(&fs, &reg_b.adapters_dir()).unwrap();
    let ms_b = compute_material_set(
        &fs,
        &reg_b,
        &adapters_b,
        &NamespaceId::new("only-project").unwrap(),
    )
    .unwrap();
    let hash_b = aenv_core::hash::hash_resolved_namespace(&ms_b);

    assert_eq!(
        hash_a, hash_b,
        "adding user_files must not change the project-scope hash"
    );
}
