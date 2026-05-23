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
