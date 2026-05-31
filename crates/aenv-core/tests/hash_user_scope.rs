//! User-scope hash tests for R-84.
//!
//! Task 23 adds `compute_material_set_user` as the symmetric sibling of
//! `compute_material_set`. These tests prove the user-scope material set
//!
//! 1. Hashes identically across two namespaces whose `aenv.toml` differs only
//!    in formatting whitespace (manifest-formatting-blindness), and
//! 2. Hashes differently when the user-scope payload bytes change.
//!
//! Project-scope hash stability when `user_files` is added is already covered
//! by `project_scope_excludes_user_files.rs::project_hash_is_stable_when_user_files_are_added`.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::RealFilesystem;
use aenv_core::hash::hash_resolved_namespace;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::compute_material_set_user;

fn setup_adapter(fs: &RealFilesystem, registry: &RegistryLayout) -> AdapterRegistry {
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
    AdapterRegistry::load_from_dir(fs, &adapters_dir).unwrap()
}

#[test]
fn user_scope_hash_equals_for_equivalent_namespaces() {
    let fs = RealFilesystem;

    // Two namespaces with the same user-scope payload but different metadata
    // formatting inside aenv.toml — the hash must be identical (R-84 is
    // manifest-formatting-blind).
    let tmp_a = tempfile::tempdir().unwrap();
    let reg_a = RegistryLayout::new(tmp_a.path().to_path_buf());
    let adapters_a = setup_adapter(&fs, &reg_a);
    let ns_a = reg_a.namespace_dir("equiv");
    std::fs::create_dir_all(ns_a.join("user/.claude")).unwrap();
    std::fs::write(ns_a.join("user/.claude/CLAUDE.md"), b"user body").unwrap();
    std::fs::write(
        ns_a.join("aenv.toml"),
        r#"
name = "equiv"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ms_a = compute_material_set_user(
        &fs,
        &reg_a,
        &adapters_a,
        &NamespaceId::new("equiv").unwrap(),
    )
    .unwrap();
    let hash_a = hash_resolved_namespace(&ms_a);

    let tmp_b = tempfile::tempdir().unwrap();
    let reg_b = RegistryLayout::new(tmp_b.path().to_path_buf());
    let adapters_b = setup_adapter(&fs, &reg_b);
    let ns_b = reg_b.namespace_dir("equiv");
    std::fs::create_dir_all(ns_b.join("user/.claude")).unwrap();
    std::fs::write(ns_b.join("user/.claude/CLAUDE.md"), b"user body").unwrap();
    // Different TOML formatting (extra whitespace/newlines, spaces inside the
    // array) — same effective content.
    std::fs::write(
        ns_b.join("aenv.toml"),
        "\n\nname = \"equiv\"\n\n\n[adapters.claude-code]\nuser_files = [ \".claude/CLAUDE.md\" ]\n",
    )
    .unwrap();
    let ms_b = compute_material_set_user(
        &fs,
        &reg_b,
        &adapters_b,
        &NamespaceId::new("equiv").unwrap(),
    )
    .unwrap();
    let hash_b = hash_resolved_namespace(&ms_b);

    assert_eq!(
        hash_a, hash_b,
        "user-scope hash must be manifest-formatting-blind"
    );
}

/// Regression: a `user_files` entry that names a *directory* (e.g. the
/// `.claude/agents/` a heuristic git import produces) must not crash the
/// material-set computation. Activation symlinks the directory as a unit, so
/// the resolved material is the directory's recursive file contents — reading
/// the directory itself as bytes previously failed with `Is a directory`.
#[test]
fn user_scope_directory_entry_expands_into_per_file_material() {
    let fs = RealFilesystem;

    let make = |files: &[(&str, &[u8])]| {
        let tmp = tempfile::tempdir().unwrap();
        let reg = RegistryLayout::new(tmp.path().to_path_buf());
        let adapters = setup_adapter(&fs, &reg);
        let ns = reg.namespace_dir("dirns");
        std::fs::create_dir_all(ns.join("user/.claude/agents")).unwrap();
        for (rel, body) in files {
            std::fs::write(ns.join("user").join(rel), body).unwrap();
        }
        std::fs::write(
            ns.join("aenv.toml"),
            r#"
name = "dirns"
[adapters.claude-code]
user_files = [".claude/agents/"]
"#,
        )
        .unwrap();
        let ms =
            compute_material_set_user(&fs, &reg, &adapters, &NamespaceId::new("dirns").unwrap())
                .unwrap();
        // `tmp` drops here, deleting the tempdir — fine, because the returned
        // material set is self-contained: owned bytes and relative paths that
        // reference no on-disk state.
        ms
    };

    let ms = make(&[
        (".claude/agents/a.md", b"agent one"),
        (".claude/agents/b.md", b"agent two"),
    ]);
    // The directory expanded into one entry per contained file (not one entry
    // for the directory itself).
    let paths: Vec<String> = ms
        .entries()
        .iter()
        .map(|(p, _)| p.to_string_lossy().replace('\\', "/"))
        .collect();
    assert!(
        paths.iter().any(|p| p == ".claude/agents/a.md")
            && paths.iter().any(|p| p == ".claude/agents/b.md"),
        "directory entry must expand into per-file material, got {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p == ".claude/agents"),
        "the directory itself must not appear as a material entry"
    );

    // Hash covers the tree: changing a file inside the directory changes it.
    let hash_a = hash_resolved_namespace(&ms);
    let ms2 = make(&[
        (".claude/agents/a.md", b"agent one CHANGED"),
        (".claude/agents/b.md", b"agent two"),
    ]);
    assert_ne!(
        hash_a,
        hash_resolved_namespace(&ms2),
        "hash must change when a file inside the directory entry changes"
    );
}

#[test]
fn user_scope_hash_changes_when_user_content_changes() {
    let fs = RealFilesystem;

    let tmp_a = tempfile::tempdir().unwrap();
    let reg_a = RegistryLayout::new(tmp_a.path().to_path_buf());
    let adapters_a = setup_adapter(&fs, &reg_a);
    let ns_a = reg_a.namespace_dir("v1");
    std::fs::create_dir_all(ns_a.join("user/.claude")).unwrap();
    std::fs::write(ns_a.join("user/.claude/CLAUDE.md"), b"version one").unwrap();
    std::fs::write(
        ns_a.join("aenv.toml"),
        r#"
name = "v1"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ms_a =
        compute_material_set_user(&fs, &reg_a, &adapters_a, &NamespaceId::new("v1").unwrap())
            .unwrap();
    let hash_a = hash_resolved_namespace(&ms_a);

    let tmp_b = tempfile::tempdir().unwrap();
    let reg_b = RegistryLayout::new(tmp_b.path().to_path_buf());
    let adapters_b = setup_adapter(&fs, &reg_b);
    let ns_b = reg_b.namespace_dir("v1");
    std::fs::create_dir_all(ns_b.join("user/.claude")).unwrap();
    std::fs::write(ns_b.join("user/.claude/CLAUDE.md"), b"version TWO").unwrap();
    std::fs::write(
        ns_b.join("aenv.toml"),
        r#"
name = "v1"
[adapters.claude-code]
user_files = [".claude/CLAUDE.md"]
"#,
    )
    .unwrap();
    let ms_b =
        compute_material_set_user(&fs, &reg_b, &adapters_b, &NamespaceId::new("v1").unwrap())
            .unwrap();
    let hash_b = hash_resolved_namespace(&ms_b);

    assert_ne!(
        hash_a, hash_b,
        "user-scope hash must change when user-scope content changes"
    );
}
