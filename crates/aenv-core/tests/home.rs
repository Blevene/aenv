//! Tests for `RegistryLayout`: derived paths under a registry root.

use aenv_core::home::RegistryLayout;
use std::path::PathBuf;

fn layout() -> RegistryLayout {
    RegistryLayout::new(PathBuf::from("/aenv"))
}

#[test]
fn namespaces_dir_is_envs_subfolder() {
    assert_eq!(layout().namespaces_dir(), PathBuf::from("/aenv/envs"));
}

#[test]
fn namespace_dir_joins_under_envs() {
    assert_eq!(
        layout().namespace_dir("experiments"),
        PathBuf::from("/aenv/envs/experiments")
    );
}

#[test]
fn manifest_path_is_namespace_aenv_toml() {
    assert_eq!(
        layout().manifest_path("experiments"),
        PathBuf::from("/aenv/envs/experiments/aenv.toml")
    );
}

#[test]
fn adapters_dir_is_adapters_subfolder() {
    assert_eq!(layout().adapters_dir(), PathBuf::from("/aenv/adapters"));
}

#[test]
fn config_path_is_root_config_toml() {
    assert_eq!(layout().config_path(), PathBuf::from("/aenv/config.toml"));
}
