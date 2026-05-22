use aenv_core::adapter::AdapterRegistry;
use aenv_core::fs::{Filesystem, MockFilesystem};
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::parameters::ParameterValue;
use aenv_core::policies::PolicyValue;
use aenv_core::resolve::resolve_namespace;
use std::path::PathBuf;

fn write_manifest(fs: &MockFilesystem, layout: &RegistryLayout, name: &str, body: &str) {
    fs.write(&layout.manifest_path(name), body.as_bytes()).unwrap();
    fs.write(
        &layout.namespace_dir(name).join("CLAUDE.md"),
        b"placeholder",
    )
    .unwrap();
}

#[test]
fn resolves_parameters_and_policies_from_chain() {
    let fs = MockFilesystem::new();
    let layout = RegistryLayout::new(PathBuf::from("/h"));
    let adapters = AdapterRegistry::new(); // empty adapters → ignore the [adapters.x] field; but resolver still walks
    // Even with empty adapter registry, the manifests below declare no adapters,
    // so resolution should succeed.

    write_manifest(
        &fs,
        &layout,
        "base",
        r#"
name = "base"

[parameters]
default_model = "haiku"
budget = 5000

[policies]
skill_requires_description = true
"#,
    );
    write_manifest(
        &fs,
        &layout,
        "leaf",
        r#"
name = "leaf"
extends = ["base"]

[parameters]
default_model = "opus"

[policies]
instructions_max_chars = { value = 3000, enforce = true }
"#,
    );

    let r = resolve_namespace(&fs, &layout, &adapters, &NamespaceId::new("leaf").unwrap()).unwrap();

    let params = r.parameters;
    let policies = r.policies;
    assert_eq!(
        params.get("default_model").unwrap().value,
        ParameterValue::String("opus".into())
    );
    assert_eq!(params.get("default_model").unwrap().source.as_str(), "leaf");
    assert_eq!(
        params.get("budget").unwrap().value,
        ParameterValue::Integer(5000)
    );
    assert_eq!(params.get("budget").unwrap().source.as_str(), "base");

    let s = policies.get("skill_requires_description").unwrap();
    assert_eq!(s.value, PolicyValue::Boolean(true));
    assert_eq!(s.source.as_str(), "base");
    let im = policies.get("instructions_max_chars").unwrap();
    assert_eq!(im.value, PolicyValue::Integer(3000));
    assert!(im.enforce);
}
