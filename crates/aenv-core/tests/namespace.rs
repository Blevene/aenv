//! Tests for namespace registry operations.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::namespace::{create_namespace, delete_namespace, list_namespaces};
use aenv_core::AenvError;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

#[test]
fn create_writes_default_manifest() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments", &[], &[]).unwrap();

    let manifest_bytes = fs.read(&layout.manifest_path("experiments")).unwrap();
    let m = AenvManifest::from_toml(&String::from_utf8(manifest_bytes).unwrap()).unwrap();
    assert_eq!(m.name, "experiments");
    assert!(m.adapters.is_empty());
}

#[test]
fn create_rejects_duplicate() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments", &[], &[]).unwrap();
    let err = create_namespace(&fs, &layout, "experiments", &[], &[]).expect_err("must reject");
    assert!(matches!(err, AenvError::ManifestInvalid(_)));
}

#[test]
fn list_returns_empty_when_no_namespaces() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert!(names.is_empty());
}

#[test]
fn list_returns_namespace_names_sorted() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments", &[], &[]).unwrap();
    create_namespace(&fs, &layout, "analyst", &[], &[]).unwrap();
    create_namespace(&fs, &layout, "detailed-execution", &[], &[]).unwrap();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert_eq!(
        names,
        vec![
            "analyst".to_string(),
            "detailed-execution".to_string(),
            "experiments".to_string(),
        ]
    );
}

#[test]
fn list_skips_entries_without_manifest() {
    // A stray directory under envs/ that lacks aenv.toml is not a namespace.
    // list_namespaces silently ignores it.
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "real", &[], &[]).unwrap();
    fs.create_dir_all(&layout.namespaces_dir().join("stray"))
        .unwrap();
    let names = list_namespaces(&fs, &layout).unwrap();
    assert_eq!(names, vec!["real".to_string()]);
}

#[test]
fn delete_removes_namespace_directory() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "experiments", &[], &[]).unwrap();
    delete_namespace(&fs, &layout, "experiments").unwrap();
    assert!(!fs.exists(&layout.namespace_dir("experiments")).unwrap());
}

#[test]
fn create_with_extends_writes_extends_list() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "base", &[], &[]).unwrap();
    create_namespace(&fs, &layout, "experiments", &["base".to_string()], &[]).unwrap();

    let manifest_bytes = fs.read(&layout.manifest_path("experiments")).unwrap();
    let m = AenvManifest::from_toml(&String::from_utf8(manifest_bytes).unwrap()).unwrap();
    assert_eq!(m.name, "experiments");
    assert_eq!(m.extends, vec!["base".to_string()]);
}

#[test]
fn create_with_multiple_extends_writes_all_parents() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "base", &[], &[]).unwrap();
    create_namespace(&fs, &layout, "shared", &[], &[]).unwrap();
    create_namespace(
        &fs,
        &layout,
        "experiments",
        &["base".to_string(), "shared".to_string()],
        &[],
    )
    .unwrap();

    let manifest_bytes = fs.read(&layout.manifest_path("experiments")).unwrap();
    let m = AenvManifest::from_toml(&String::from_utf8(manifest_bytes).unwrap()).unwrap();
    assert_eq!(m.extends, vec!["base".to_string(), "shared".to_string()]);
}

#[test]
fn delete_rejects_unknown_namespace() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let err = delete_namespace(&fs, &layout, "nope").expect_err("must error");
    assert!(matches!(err, AenvError::NamespaceNotFound(_)));
    assert_eq!(err.exit_code(), 10);
}

#[test]
fn create_namespace_with_single_adapter() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(&fs, &layout, "foo", &[], &["claude-code".to_string()]).unwrap();
    let bytes = fs.read(&layout.manifest_path("foo")).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("[adapters.claude-code]"), "manifest: {text}");
}

#[test]
fn create_namespace_with_multiple_adapters() {
    let fs = MockFilesystem::new();
    let layout = layout();
    create_namespace(
        &fs,
        &layout,
        "foo",
        &[],
        &["claude-code".to_string(), "cursor".to_string()],
    )
    .unwrap();
    let text = String::from_utf8(fs.read(&layout.manifest_path("foo")).unwrap()).unwrap();
    assert!(text.contains("[adapters.claude-code]"), "manifest: {text}");
    assert!(text.contains("[adapters.cursor]"), "manifest: {text}");
}

#[test]
fn fork_name_copies_managed_files_from_project_and_writes_manifest() {
    use aenv_core::adapter::{Adapter, AdapterRegistry};
    use aenv_core::namespace::create_namespace_from_project;
    use std::path::Path;

    let fs = MockFilesystem::new();

    fs.write(Path::new("/p/CLAUDE.md"), b"# project version\n")
        .unwrap();
    fs.write(Path::new("/p/.mcp.json"), b"{}").unwrap();

    let cc: Adapter = toml::from_str("name = \"claude-code\"\nfiles = [\"CLAUDE.md\"]\n").unwrap();
    let mcp: Adapter = toml::from_str("name = \"mcp\"\nfiles = [\".mcp.json\"]\n").unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc);
    adapters.insert(mcp);

    let reg = RegistryLayout::new(PathBuf::from("/aenv"));
    create_namespace_from_project(&fs, &reg, &adapters, "new-env", Path::new("/p"), &[]).unwrap();

    let manifest_bytes = fs.read(Path::new("/aenv/envs/new-env/aenv.toml")).unwrap();
    let m: aenv_core::manifest::AenvManifest =
        toml::from_str(std::str::from_utf8(&manifest_bytes).unwrap()).unwrap();
    assert_eq!(m.name, "new-env");
    assert!(m.adapters.contains_key("claude-code"));
    assert!(m.adapters.contains_key("mcp"));

    let copied_claude = fs.read(Path::new("/aenv/envs/new-env/CLAUDE.md")).unwrap();
    assert_eq!(copied_claude, b"# project version\n");
    let copied_mcp = fs.read(Path::new("/aenv/envs/new-env/.mcp.json")).unwrap();
    assert_eq!(copied_mcp, b"{}");
}

#[test]
fn fork_name_walks_glob_directories_and_copies_every_file() {
    use aenv_core::adapter::{Adapter, AdapterRegistry};
    use aenv_core::namespace::create_namespace_from_project;
    use std::path::Path;

    let fs = MockFilesystem::new();
    fs.write(Path::new("/p/.claude/skills/a/SKILL.md"), b"skill a")
        .unwrap();
    fs.write(Path::new("/p/.claude/skills/b/SKILL.md"), b"skill b")
        .unwrap();
    fs.write(Path::new("/p/CLAUDE.md"), b"# proj\n").unwrap();

    let cc: Adapter = toml::from_str(
        "name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/skills/**/*\"]\n",
    )
    .unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc);

    let reg = RegistryLayout::new(PathBuf::from("/aenv"));
    create_namespace_from_project(&fs, &reg, &adapters, "forked", Path::new("/p"), &[]).unwrap();

    assert_eq!(
        fs.read(Path::new("/aenv/envs/forked/.claude/skills/a/SKILL.md"))
            .unwrap(),
        b"skill a",
    );
    assert_eq!(
        fs.read(Path::new("/aenv/envs/forked/.claude/skills/b/SKILL.md"))
            .unwrap(),
        b"skill b",
    );

    let body = fs.read(Path::new("/aenv/envs/forked/aenv.toml")).unwrap();
    let m: aenv_core::manifest::AenvManifest =
        toml::from_str(std::str::from_utf8(&body).unwrap()).unwrap();
    let files = &m.adapters["claude-code"].files;
    assert!(files.iter().any(|p| p == "CLAUDE.md"));
    assert!(files.iter().any(|p| p == ".claude/skills/a/SKILL.md"));
    assert!(files.iter().any(|p| p == ".claude/skills/b/SKILL.md"));
    assert!(!files.iter().any(|p| p.contains('*')));
}

#[test]
fn fork_name_walks_trailing_slash_directory_marker() {
    // Regression: the shipped claude-code adapter declares `.claude/` (no
    // glob) as one of its files. Snapshot used to treat that as a literal
    // path, call fs.read() on a directory, and crash with EISDIR. The fix
    // recognizes a trailing-slash entry as a directory walk, the same way
    // `.cursor/**/*` is.
    use aenv_core::adapter::{Adapter, AdapterRegistry};
    use aenv_core::namespace::create_namespace_from_project;
    use std::path::Path;

    let fs = MockFilesystem::new();
    fs.write(Path::new("/p/CLAUDE.md"), b"# proj\n").unwrap();
    fs.write(Path::new("/p/.claude/skills/notes/SKILL.md"), b"notes")
        .unwrap();
    fs.write(Path::new("/p/.claude/agents/planner.md"), b"planner")
        .unwrap();

    // Exactly the shipped claude-code adapter declaration.
    let cc: Adapter =
        toml::from_str("name = \"claude-code\"\nfiles = [\"CLAUDE.md\", \".claude/\"]\n").unwrap();
    let mut adapters = AdapterRegistry::default();
    adapters.insert(cc);

    let reg = RegistryLayout::new(PathBuf::from("/aenv"));
    create_namespace_from_project(
        &fs,
        &reg,
        &adapters,
        "from-trailing-slash",
        Path::new("/p"),
        &[],
    )
    .unwrap();

    let body = fs
        .read(Path::new("/aenv/envs/from-trailing-slash/aenv.toml"))
        .unwrap();
    let m: aenv_core::manifest::AenvManifest =
        toml::from_str(std::str::from_utf8(&body).unwrap()).unwrap();
    let files = &m.adapters["claude-code"].files;
    assert!(files.iter().any(|p| p == "CLAUDE.md"));
    assert!(files.iter().any(|p| p == ".claude/skills/notes/SKILL.md"));
    assert!(files.iter().any(|p| p == ".claude/agents/planner.md"));
    // Expanded into concrete paths — no trailing-slash markers in the output.
    assert!(!files.iter().any(|p| p.ends_with('/')));
}
