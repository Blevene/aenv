use aenv_core::adapter::AdapterRegistry;
use aenv_core::diff::structural;
use aenv_core::home::RegistryLayout;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn setup() -> (TempDir, RegistryLayout, AdapterRegistry) {
    let tmp = TempDir::new().unwrap();
    let layout = RegistryLayout::new(tmp.path().to_path_buf());
    let fs = aenv_core::fs::RealFilesystem;
    std::fs::create_dir_all(layout.adapters_dir()).unwrap();
    aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir()).unwrap();
    let adapters = AdapterRegistry::load_from_dir(&fs, &layout.adapters_dir()).unwrap();
    (tmp, layout, adapters)
}

#[test]
fn structural_diff_reports_skill_roster_difference() {
    let (_tmp, layout, adapters) = setup();
    write_file(
        &layout.manifest_path("alpha"),
        r#"name = "alpha"
[adapters.claude-code]
files = []
[[skills]]
name = "a"
mode = "authored"
adapter = "claude-code"
"#,
    );
    write_file(
        &layout.manifest_path("beta"),
        r#"name = "beta"
[adapters.claude-code]
files = []
[[skills]]
name = "b"
mode = "authored"
adapter = "claude-code"
"#,
    );
    let fs = aenv_core::fs::RealFilesystem;
    let diff = structural(&fs, &layout, &adapters, "alpha", "beta").unwrap();
    assert_eq!(diff.a, "alpha");
    assert_eq!(diff.b, "beta");
    assert_eq!(diff.skills.added, vec!["beta::b".to_string()]);
    assert_eq!(diff.skills.removed, vec!["alpha::a".to_string()]);
    assert!(diff.skills.common.is_empty());
}

#[test]
fn structural_diff_reports_parameter_value_changes() {
    let (_tmp, layout, adapters) = setup();
    write_file(
        &layout.manifest_path("alpha"),
        r#"name = "alpha"
[adapters.claude-code]
files = []
[parameters]
default_model = "claude-sonnet-4.6"
"#,
    );
    write_file(
        &layout.manifest_path("beta"),
        r#"name = "beta"
[adapters.claude-code]
files = []
[parameters]
default_model = "claude-opus-4.7"
"#,
    );
    let fs = aenv_core::fs::RealFilesystem;
    let diff = structural(&fs, &layout, &adapters, "alpha", "beta").unwrap();
    assert_eq!(diff.parameters.changed.len(), 1);
    assert_eq!(diff.parameters.changed[0].name, "default_model");
    assert_eq!(
        diff.parameters.changed[0].a,
        serde_json::json!("claude-sonnet-4.6")
    );
    assert_eq!(
        diff.parameters.changed[0].b,
        serde_json::json!("claude-opus-4.7")
    );
}

#[test]
fn structural_diff_reports_section_body_differs() {
    let (_tmp, layout, adapters) = setup();
    // Both namespaces have a ## Disposition section but with different bodies.
    write_file(
        &layout.manifest_path("alpha"),
        "name = \"alpha\"\n\
         [adapters.claude-code]\n\
         files = [\"CLAUDE.md\"]\n",
    );
    write_file(
        &layout.namespace_dir("alpha").join("CLAUDE.md"),
        "## Disposition\nAlpha emphasizes breadth.\n",
    );
    write_file(
        &layout.manifest_path("beta"),
        "name = \"beta\"\n\
         [adapters.claude-code]\n\
         files = [\"CLAUDE.md\"]\n",
    );
    write_file(
        &layout.namespace_dir("beta").join("CLAUDE.md"),
        "## Disposition\nBeta emphasizes care and detailed execution.\n",
    );

    let fs = aenv_core::fs::RealFilesystem;
    let diff = structural(&fs, &layout, &adapters, "alpha", "beta").unwrap();

    assert_eq!(diff.instructions_sections.common, vec!["Disposition"]);
    assert_eq!(diff.instructions_section_diffs.len(), 1);
    let delta = &diff.instructions_section_diffs[0];
    assert_eq!(delta.heading, "Disposition");
    assert_eq!(delta.status, "differs");
    assert!(
        delta.summary.is_some(),
        "summary should be present when bodies differ"
    );
}

#[test]
fn structural_diff_reports_section_body_identical() {
    let (_tmp, layout, adapters) = setup();
    // Both namespaces have an identical ## Project Facts section.
    write_file(
        &layout.manifest_path("alpha"),
        "name = \"alpha\"\n\
         [adapters.claude-code]\n\
         files = [\"CLAUDE.md\"]\n",
    );
    write_file(
        &layout.namespace_dir("alpha").join("CLAUDE.md"),
        "## Project Facts\nShared project facts.\n",
    );
    write_file(
        &layout.manifest_path("beta"),
        "name = \"beta\"\n\
         [adapters.claude-code]\n\
         files = [\"CLAUDE.md\"]\n",
    );
    write_file(
        &layout.namespace_dir("beta").join("CLAUDE.md"),
        "## Project Facts\nShared project facts.\n",
    );

    let fs = aenv_core::fs::RealFilesystem;
    let diff = structural(&fs, &layout, &adapters, "alpha", "beta").unwrap();

    assert_eq!(diff.instructions_sections.common, vec!["Project Facts"]);
    assert_eq!(diff.instructions_section_diffs.len(), 1);
    let delta = &diff.instructions_section_diffs[0];
    assert_eq!(delta.heading, "Project Facts");
    assert_eq!(delta.status, "identical");
    assert!(
        delta.summary.is_none(),
        "summary should be None when bodies are identical"
    );
}

#[test]
fn structural_diff_no_common_sections_means_empty_deltas() {
    let (_tmp, layout, adapters) = setup();
    // No overlap in section headings.
    write_file(
        &layout.manifest_path("alpha"),
        "name = \"alpha\"\n\
         [adapters.claude-code]\n\
         files = [\"CLAUDE.md\"]\n",
    );
    write_file(
        &layout.namespace_dir("alpha").join("CLAUDE.md"),
        "## Only In Alpha\nSome content.\n",
    );
    write_file(
        &layout.manifest_path("beta"),
        "name = \"beta\"\n\
         [adapters.claude-code]\n\
         files = [\"CLAUDE.md\"]\n",
    );
    write_file(
        &layout.namespace_dir("beta").join("CLAUDE.md"),
        "## Only In Beta\nOther content.\n",
    );

    let fs = aenv_core::fs::RealFilesystem;
    let diff = structural(&fs, &layout, &adapters, "alpha", "beta").unwrap();

    assert!(diff.instructions_sections.common.is_empty());
    assert!(diff.instructions_section_diffs.is_empty());
}
