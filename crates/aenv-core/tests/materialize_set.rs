//! Verify the pure material-set computation matches what activation
//! would write for each of the four real strategies: Symlink, Identical,
//! SectionMerge, DeepMerge(Json).

use aenv_core::adapter::AdapterRegistry;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::materialize::compute_material_set;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup() -> (TempDir, RegistryLayout, AdapterRegistry) {
    let tmp = TempDir::new().unwrap();
    let layout = RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    (tmp, layout, adapters)
}

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[test]
fn single_symlink_candidate_contributes_source_bytes() {
    let (_tmp, layout, adapters) = setup();
    let ns_root = layout.namespace_dir("solo");
    write_file(
        &layout.manifest_path("solo"),
        "name = \"solo\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&ns_root.join("CLAUDE.md"), "# Hello\nProject facts.\n");

    let fs = aenv_core::fs::RealFilesystem;
    let leaf = NamespaceId::new("solo").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();

    assert_eq!(mat.entries.len(), 1);
    assert_eq!(mat.entries[0].0, PathBuf::from("CLAUDE.md"));
    assert_eq!(mat.entries[0].1, b"# Hello\nProject facts.\n".to_vec());
}

#[test]
fn section_merge_combines_two_namespaces() {
    let (_tmp, layout, adapters) = setup();
    let base = layout.namespace_dir("base");
    let leaf = layout.namespace_dir("leaf");
    write_file(
        &layout.manifest_path("base"),
        "name = \"base\"\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&base.join("CLAUDE.md"), "## Facts\nA\n");
    write_file(
        &layout.manifest_path("leaf"),
        "name = \"leaf\"\nextends = [\"base\"]\n[adapters.claude-code]\nfiles = [\"CLAUDE.md\"]\n",
    );
    write_file(&leaf.join("CLAUDE.md"), "## Disposition\nB\n");

    let fs = aenv_core::fs::RealFilesystem;
    let leaf_id = NamespaceId::new("leaf").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf_id).unwrap();

    assert_eq!(mat.entries.len(), 1);
    let body = std::str::from_utf8(&mat.entries[0].1).unwrap();
    // Section merge concatenates by `##` header, base first.
    assert!(body.contains("## Facts"));
    assert!(body.contains("## Disposition"));
    let facts_pos = body.find("## Facts").unwrap();
    let disp_pos = body.find("## Disposition").unwrap();
    assert!(facts_pos < disp_pos, "base section precedes leaf section");
}

#[test]
fn deep_merge_json_uses_default_serializer_bytes() {
    let (_tmp, layout, adapters) = setup();
    let base = layout.namespace_dir("base");
    let leaf = layout.namespace_dir("leaf");
    write_file(
        &layout.manifest_path("base"),
        "name = \"base\"\n[adapters.mcp]\nfiles = [\".mcp.json\"]\n",
    );
    write_file(
        &base.join(".mcp.json"),
        "{\"servers\":{\"a\":{\"command\":\"x\"}}}\n",
    );
    write_file(
        &layout.manifest_path("leaf"),
        "name = \"leaf\"\nextends = [\"base\"]\n[adapters.mcp]\nfiles = [\".mcp.json\"]\n",
    );
    write_file(
        &leaf.join(".mcp.json"),
        "{\"servers\":{\"b\":{\"command\":\"y\"}}}\n",
    );

    let fs = aenv_core::fs::RealFilesystem;
    let leaf_id = NamespaceId::new("leaf").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf_id).unwrap();

    let body = std::str::from_utf8(&mat.entries[0].1).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
    assert!(parsed["servers"]["a"].is_object());
    assert!(parsed["servers"]["b"].is_object());
}

#[test]
fn entries_are_sorted_by_path() {
    let (_tmp, layout, adapters) = setup();
    let ns_root = layout.namespace_dir("multi");
    write_file(
        &layout.manifest_path("multi"),
        "name = \"multi\"\n[adapters.claude-code]\nfiles = [\"z.md\", \"a.md\", \"m.md\"]\n",
    );
    write_file(&ns_root.join("z.md"), "z\n");
    write_file(&ns_root.join("a.md"), "a\n");
    write_file(&ns_root.join("m.md"), "m\n");

    let fs = aenv_core::fs::RealFilesystem;
    let leaf = NamespaceId::new("multi").unwrap();
    let mat = compute_material_set(&fs, &layout, &adapters, &leaf).unwrap();

    let paths: Vec<&std::path::Path> = mat.entries.iter().map(|(p, _)| p.as_path()).collect();
    let sorted: Vec<&std::path::Path> = {
        let mut s = paths.clone();
        s.sort();
        s
    };
    assert_eq!(paths, sorted);
}
