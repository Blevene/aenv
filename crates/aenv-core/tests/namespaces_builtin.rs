//! Tests for built-in default namespaces.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::namespaces_builtin::{ensure_written, ALL};
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

#[test]
fn ships_karpathy_cherny_blank() {
    let names: Vec<&str> = ALL.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, vec!["karpathy", "cherny", "blank"]);
}

#[test]
fn every_builtin_manifest_parses_and_name_matches() {
    for (name, files) in ALL {
        let body = files
            .iter()
            .find_map(|(rel, body)| (*rel == "aenv.toml").then_some(*body))
            .unwrap_or_else(|| panic!("namespace {name} ships no aenv.toml"));
        let m = AenvManifest::from_toml(body)
            .unwrap_or_else(|e| panic!("namespace {name} manifest fails to parse: {e}"));
        assert_eq!(m.name, *name);
        assert!(
            m.adapters.contains_key("claude-code"),
            "namespace {name} should declare the claude-code adapter"
        );
    }
}

#[test]
fn ensure_written_writes_every_file_for_every_namespace() {
    let fs = MockFilesystem::new();
    let layout = layout();
    ensure_written(&fs, &layout).unwrap();
    for (name, files) in ALL {
        for (rel, _) in *files {
            let path = layout.namespace_dir(name).join(rel);
            assert!(fs.exists(&path).unwrap(), "missing {}", path.display());
        }
    }
}

#[test]
fn ensure_written_leaves_user_edits_alone() {
    let fs = MockFilesystem::new();
    let layout = layout();
    let edited = layout.namespace_dir("karpathy").join("CLAUDE.md");
    fs.write(&edited, b"user-customized\n").unwrap();
    ensure_written(&fs, &layout).unwrap();
    let body = String::from_utf8(fs.read(&edited).unwrap()).unwrap();
    assert_eq!(body, "user-customized\n");
}

#[test]
fn ensure_written_is_idempotent() {
    let fs = MockFilesystem::new();
    let layout = layout();
    ensure_written(&fs, &layout).unwrap();
    ensure_written(&fs, &layout).unwrap();
    let manifest = layout.manifest_path("cherny");
    assert!(fs.exists(&manifest).unwrap());
}
