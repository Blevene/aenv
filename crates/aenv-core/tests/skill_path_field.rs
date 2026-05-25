//! Tests for the `[[skills]].path` field — monorepo sub-path support.

use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::local::resolve_local;
use aenv_core::skills::{resolve_imported_skill, SkillDecl, SkillMode};
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

#[test]
fn path_roundtrips_through_toml() {
    let body = r#"
name = "scientific"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "scanpy"
mode = "imported"
adapter = "claude-code"
source = "git+https://github.com/example/repo"
path = "scientific-skills/scanpy"
"#;
    let m = AenvManifest::from_toml(body).expect("parse");
    assert_eq!(m.skills.len(), 1);
    assert_eq!(
        m.skills[0].path.as_deref(),
        Some("scientific-skills/scanpy")
    );
    let serialized = m.to_toml();
    let m2 = AenvManifest::from_toml(&serialized).expect("re-parse");
    assert_eq!(m.skills[0], m2.skills[0]);
}

#[test]
fn path_rejected_on_authored_skill() {
    let body = r#"
name = "ns"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "x"
mode = "authored"
adapter = "claude-code"
path = "subdir"
"#;
    let err = AenvManifest::from_toml(body).unwrap_err().to_string();
    assert!(err.contains("authored"), "got: {err}");
    assert!(err.contains("path"), "got: {err}");
}

#[test]
fn absolute_path_rejected() {
    let body = r#"
name = "ns"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "x"
mode = "imported"
adapter = "claude-code"
source = "/local/repo"
path = "/etc/passwd"
"#;
    let err = AenvManifest::from_toml(body).unwrap_err().to_string();
    assert!(err.contains("relative"), "got: {err}");
}

#[test]
fn dot_dot_path_rejected() {
    let body = r#"
name = "ns"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "x"
mode = "imported"
adapter = "claude-code"
source = "/local/repo"
path = "scientific-skills/../../etc"
"#;
    let err = AenvManifest::from_toml(body).unwrap_err().to_string();
    assert!(err.contains(".."), "got: {err}");
}

#[test]
fn empty_path_rejected() {
    let body = r#"
name = "ns"

[adapters.claude-code]
files = ["CLAUDE.md"]

[[skills]]
name = "x"
mode = "imported"
adapter = "claude-code"
source = "/local/repo"
path = ""
"#;
    let err = AenvManifest::from_toml(body).unwrap_err().to_string();
    assert!(err.contains("empty"), "got: {err}");
}

#[test]
fn resolve_local_with_subpath_picks_only_that_skill() {
    let fs = MockFilesystem::new();
    // Monorepo layout: two skills side-by-side under /repo/scientific-skills/
    fs.write(
        std::path::Path::new("/repo/scientific-skills/scanpy/SKILL.md"),
        b"---\nname: scanpy\ndescription: scRNA\n---\n",
    )
    .unwrap();
    fs.write(
        std::path::Path::new("/repo/scientific-skills/rdkit/SKILL.md"),
        b"---\nname: rdkit\ndescription: chem\n---\n",
    )
    .unwrap();

    let resolved = resolve_local(
        &fs,
        std::path::Path::new("/repo"),
        "scanpy",
        Some("scientific-skills/scanpy"),
    )
    .unwrap();
    // source_path is the sub-dir, NOT /repo — so the materialization walk
    // won't pull in rdkit too.
    assert_eq!(
        resolved.source_path,
        PathBuf::from("/repo/scientific-skills/scanpy")
    );
}

#[test]
fn resolve_local_missing_subpath_is_a_clear_error() {
    let fs = MockFilesystem::new();
    fs.write(
        std::path::Path::new("/repo/scientific-skills/scanpy/SKILL.md"),
        b"---\nname: scanpy\n---\n",
    )
    .unwrap();
    let err = resolve_local(
        &fs,
        std::path::Path::new("/repo"),
        "ghost",
        Some("scientific-skills/ghost"),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("scientific-skills/ghost") || err.contains("sub-directory"),
        "got: {err}"
    );
}

#[test]
fn resolve_imported_skill_passes_path_through() {
    let fs = MockFilesystem::new();
    fs.write(
        std::path::Path::new("/local/repo/scientific-skills/biopython/SKILL.md"),
        b"---\nname: biopython\n---\n",
    )
    .unwrap();

    let decl = SkillDecl {
        name: "biopython".into(),
        mode: SkillMode::Imported,
        adapter: Some("claude-code".into()),
        source: Some("/local/repo".into()),
        ref_: None,
        path: Some("scientific-skills/biopython".into()),
        required: false,
    };
    let r = resolve_imported_skill(&fs, &layout(), &decl).unwrap();
    assert_eq!(
        r.source_path,
        PathBuf::from("/local/repo/scientific-skills/biopython")
    );
}
