//! Resolver tests. `resolve_namespace` walks the `extends` chain, gathers
//! candidate artifacts, and returns the chain + an indexed candidate set.

use std::path::{Path, PathBuf};

use aenv_core::adapter::{Adapter, AdapterRegistry};
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::{resolve_namespace, ResolutionError};

const REG: &str = "/aenv";

fn registry() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from(REG))
}

fn write_manifest(fs: &MockFilesystem, name: &str, body: &str) {
    let path = format!("{REG}/envs/{name}/aenv.toml");
    fs.write(Path::new(&path), body.as_bytes()).unwrap();
}

fn write_file(fs: &MockFilesystem, ns: &str, rel: &str, contents: &str) {
    let path = format!("{REG}/envs/{ns}/{rel}");
    fs.write(Path::new(&path), contents.as_bytes()).unwrap();
}

fn cc_adapter() -> Adapter {
    toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md", ".claude/skills/**/*"]
"#,
    )
    .unwrap()
}

fn registry_with_cc() -> AdapterRegistry {
    let mut r = AdapterRegistry::default();
    r.insert(cc_adapter());
    r
}

#[test]
fn resolves_single_namespace_with_no_extends() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "base",
        r#"
name = "base"
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    );
    write_file(&fs, "base", "CLAUDE.md", "# base instructions\n");

    let resolved = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("base").unwrap(),
    )
    .unwrap();

    assert_eq!(resolved.chain, vec![NamespaceId::new("base").unwrap()]);
    assert!(resolved
        .candidates
        .iter()
        .any(|c| c.path == Path::new("CLAUDE.md")));
}

#[test]
fn resolves_two_level_chain_root_then_leaf() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "base",
        r#"
name = "base"
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    );
    write_file(&fs, "base", "CLAUDE.md", "# base\n");
    write_manifest(
        &fs,
        "leaf",
        r#"
name = "leaf"
extends = ["base"]
[adapters.claude-code]
files = ["CLAUDE.md"]
"#,
    );
    write_file(&fs, "leaf", "CLAUDE.md", "# leaf\n");

    let resolved = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("leaf").unwrap(),
    )
    .unwrap();
    assert_eq!(
        resolved.chain,
        vec![
            NamespaceId::new("base").unwrap(),
            NamespaceId::new("leaf").unwrap()
        ]
    );
    assert_eq!(resolved.candidates.len(), 2);
    assert_eq!(resolved.candidates[0].namespace.as_str(), "base");
    assert_eq!(resolved.candidates[1].namespace.as_str(), "leaf");
}

#[test]
fn detects_two_node_cycle() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "a",
        r#"
name = "a"
extends = ["b"]
"#,
    );
    write_manifest(
        &fs,
        "b",
        r#"
name = "b"
extends = ["a"]
"#,
    );
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("a").unwrap(),
    )
    .unwrap_err();
    match err {
        ResolutionError::Cycle(chain) => {
            assert_eq!(chain.first().unwrap().as_str(), "a");
            assert_eq!(chain.last().unwrap().as_str(), "a");
            assert!(chain.iter().any(|n| n.as_str() == "b"));
        }
        other => panic!("expected Cycle, got {other:?}"),
    }
}

#[test]
fn detects_self_cycle() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "selfish",
        r#"
name = "selfish"
extends = ["selfish"]
"#,
    );
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("selfish").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, ResolutionError::Cycle(_)));
}

#[test]
fn resolves_diamond_without_duplication() {
    let fs = MockFilesystem::new();
    write_manifest(&fs, "shared", r#"name = "shared""#);
    write_manifest(
        &fs,
        "left",
        r#"
name = "left"
extends = ["shared"]
"#,
    );
    write_manifest(
        &fs,
        "right",
        r#"
name = "right"
extends = ["shared"]
"#,
    );
    write_manifest(
        &fs,
        "top",
        r#"
name = "top"
extends = ["left", "right"]
"#,
    );
    let resolved = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("top").unwrap(),
    )
    .unwrap();
    let count_shared = resolved
        .chain
        .iter()
        .filter(|n| n.as_str() == "shared")
        .count();
    assert_eq!(count_shared, 1);
    assert_eq!(
        resolved
            .chain
            .iter()
            .map(aenv_core::identity::NamespaceId::as_str)
            .collect::<Vec<_>>(),
        vec!["shared", "left", "right", "top"]
    );
}

#[test]
fn rejects_unknown_namespace() {
    let fs = MockFilesystem::new();
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("missing").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, ResolutionError::NamespaceNotFound(_)));
}

#[test]
fn rejects_manifest_name_directory_mismatch() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "alpha",
        r#"
name = "beta"
"#,
    );
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("alpha").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, ResolutionError::ManifestInvalid { .. }));
}

#[test]
fn rejects_reference_to_missing_adapter() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "ghost",
        r#"
name = "ghost"
[adapters.does-not-exist]
files = ["foo"]
"#,
    );
    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("ghost").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(err, ResolutionError::AdapterMissing(_)));
}

#[test]
fn resolve_rejects_absolute_candidate_path() {
    // A manifest declaring an absolute path in files = [...] must be rejected
    // with ManifestInvalid. An absolute path would make the hash
    // machine-specific and silently different across environments.
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "escape",
        r#"
name = "escape"
[adapters.claude-code]
files = ["/etc/passwd"]
"#,
    );
    // Write the file at the absolute source path so gather_candidates doesn't
    // skip it on the existence check — we want the validator to fire, not the
    // skip-if-missing short-circuit.
    fs.write(Path::new("/etc/passwd"), b"root:x:0:0\n").unwrap();

    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("escape").unwrap(),
    )
    .unwrap_err();
    match &err {
        ResolutionError::ManifestInvalid { reason, .. } => {
            assert!(
                reason.contains("absolute"),
                "expected 'absolute' in error reason, got: {reason}"
            );
        }
        other => panic!("expected ManifestInvalid, got {other:?}"),
    }
}

#[test]
fn resolve_rejects_dot_dot_in_candidate_path() {
    // A manifest declaring a path with '..' traversal must be rejected with
    // ManifestInvalid. '..' in a hash input is a security and determinism risk.
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "traversal",
        r#"
name = "traversal"
[adapters.claude-code]
files = ["../escape.md"]
"#,
    );
    // The source path gather_candidates computes is ns_root.join("../escape.md")
    // = /aenv/envs/traversal/../escape.md. MockFilesystem stores files by exact
    // path string without normalizing, so we must write to the same un-normalized
    // path that exists() will look up — otherwise the candidate is silently skipped
    // and the validator never fires.
    fs.write(Path::new("/aenv/envs/traversal/../escape.md"), b"secret\n")
        .unwrap();

    let err = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_cc(),
        &NamespaceId::new("traversal").unwrap(),
    )
    .unwrap_err();
    match &err {
        ResolutionError::ManifestInvalid { reason, .. } => {
            assert!(
                reason.contains(".."),
                "expected '..' in error reason, got: {reason}"
            );
        }
        other => panic!("expected ManifestInvalid, got {other:?}"),
    }
}

// --- shared_files (issue #5 Layer 2) -------------------------------------

/// claude-code-like adapter with the role maps shared_files remapping needs:
/// the instructions file is asymmetric (`CLAUDE.md` project vs
/// `~/.claude/CLAUDE.md` user).
fn cc_adapter_with_roles() -> Adapter {
    toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md", ".claude/"]
user_files = ["~/.claude/CLAUDE.md", "~/.claude/agents/"]

[roles]
"CLAUDE.md" = "instructions"

[user_roles]
"~/.claude/CLAUDE.md" = "instructions"
"#,
    )
    .unwrap()
}

fn registry_with_roles() -> AdapterRegistry {
    let mut r = AdapterRegistry::default();
    r.insert(cc_adapter_with_roles());
    r
}

#[test]
fn shared_files_emit_user_and_role_remapped_project_candidates() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "shareprof",
        r#"
name = "shareprof"
[adapters.claude-code]
shared_files = [".claude/CLAUDE.md", ".claude/agents/helper.md"]
"#,
    );
    // One stored copy, under user/.
    write_file(&fs, "shareprof", "user/.claude/CLAUDE.md", "# shared\n");
    write_file(
        &fs,
        "shareprof",
        "user/.claude/agents/helper.md",
        "helper\n",
    );

    let resolved = resolve_namespace(
        &fs,
        &registry(),
        &registry_with_roles(),
        &NamespaceId::new("shareprof").unwrap(),
    )
    .unwrap();

    let find = |scope: aenv_core::scope::Scope, path: &str| {
        resolved
            .candidates
            .iter()
            .find(|c| c.scope == scope && c.path == Path::new(path))
            .unwrap_or_else(|| panic!("missing {scope:?} candidate for {path}"))
    };

    // The role-tagged instructions file: user keeps the .claude/ layout, the
    // project destination is remapped to repo-root CLAUDE.md. Both read the
    // SAME single source under user/.
    let user_md = find(aenv_core::scope::Scope::User, ".claude/CLAUDE.md");
    let proj_md = find(aenv_core::scope::Scope::Project, "CLAUDE.md");
    let expected_src = Path::new("/aenv/envs/shareprof/user/.claude/CLAUDE.md");
    assert_eq!(user_md.source_path, expected_src);
    assert_eq!(proj_md.source_path, expected_src);
    // No project candidate at the un-remapped user layout path.
    assert!(
        !resolved
            .candidates
            .iter()
            .any(|c| c.scope == aenv_core::scope::Scope::Project
                && c.path == Path::new(".claude/CLAUDE.md")),
        "instructions file must remap, not pass through, for project scope"
    );

    // A non-role file is symmetric: identical path in both scopes, one source.
    let user_h = find(aenv_core::scope::Scope::User, ".claude/agents/helper.md");
    let proj_h = find(aenv_core::scope::Scope::Project, ".claude/agents/helper.md");
    assert_eq!(user_h.source_path, proj_h.source_path);
}

#[test]
fn shared_files_ambiguous_role_is_manifest_invalid() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "amb",
        r#"
name = "amb"
[adapters.claude-code]
shared_files = [".claude/CLAUDE.md"]
"#,
    );
    write_file(&fs, "amb", "user/.claude/CLAUDE.md", "# x\n");

    // Adapter assigns the `instructions` role to TWO project paths.
    let adapter: Adapter = toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md", "AGENTS.md"]
user_files = ["~/.claude/CLAUDE.md"]
[roles]
"CLAUDE.md" = "instructions"
"AGENTS.md" = "instructions"
[user_roles]
"~/.claude/CLAUDE.md" = "instructions"
"#,
    )
    .unwrap();
    let mut reg = AdapterRegistry::default();
    reg.insert(adapter);

    let err =
        resolve_namespace(&fs, &registry(), &reg, &NamespaceId::new("amb").unwrap()).unwrap_err();
    match &err {
        ResolutionError::ManifestInvalid { reason, .. } => {
            assert!(reason.contains("multiple"), "got: {reason}");
        }
        other => panic!("expected ManifestInvalid, got {other:?}"),
    }
}

#[test]
fn shared_files_user_role_with_no_project_path_is_manifest_invalid() {
    let fs = MockFilesystem::new();
    write_manifest(
        &fs,
        "norole",
        r#"
name = "norole"
[adapters.claude-code]
shared_files = [".claude/CLAUDE.md"]
"#,
    );
    write_file(&fs, "norole", "user/.claude/CLAUDE.md", "# x\n");

    // user_roles tags the file `instructions`, but `roles` declares no such role.
    let adapter: Adapter = toml::from_str(
        r#"
name = "claude-code"
files = ["CLAUDE.md"]
user_files = ["~/.claude/CLAUDE.md"]
[user_roles]
"~/.claude/CLAUDE.md" = "instructions"
"#,
    )
    .unwrap();
    let mut reg = AdapterRegistry::default();
    reg.insert(adapter);

    let err = resolve_namespace(&fs, &registry(), &reg, &NamespaceId::new("norole").unwrap())
        .unwrap_err();
    match &err {
        ResolutionError::ManifestInvalid { reason, .. } => {
            assert!(reason.contains("no project-scope path"), "got: {reason}");
        }
        other => panic!("expected ManifestInvalid, got {other:?}"),
    }
}
