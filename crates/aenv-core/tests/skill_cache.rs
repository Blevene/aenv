use aenv_core::home::RegistryLayout;
use aenv_core::skills::cache::{skill_cache_path, source_hash};
use std::path::PathBuf;

#[test]
fn source_hash_is_deterministic() {
    let h1 = source_hash("git+https://example.com/foo.git#main");
    let h2 = source_hash("git+https://example.com/foo.git#main");
    assert_eq!(h1, h2);
}

#[test]
fn source_hash_differs_for_different_sources() {
    let h1 = source_hash("git+https://example.com/foo.git#main");
    let h2 = source_hash("git+https://example.com/foo.git#feature");
    assert_ne!(h1, h2);
}

#[test]
fn source_hash_is_16_hex_chars() {
    let h = source_hash("anything");
    assert_eq!(h.len(), 16);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn cache_path_for_pinned_ref() {
    let layout = RegistryLayout::new(PathBuf::from("/home/u/.aenv"));
    let p = skill_cache_path(&layout, "git+https://example.com/foo.git", "v1.2.0");
    let hash = source_hash("git+https://example.com/foo.git");
    assert_eq!(
        p,
        PathBuf::from(format!("/home/u/.aenv/cache/skills/{hash}/v1.2.0"))
    );
}

#[test]
fn cache_path_for_unpinned_head() {
    let layout = RegistryLayout::new(PathBuf::from("/home/u/.aenv"));
    let p = skill_cache_path(&layout, "/local/path", "head");
    let hash = source_hash("/local/path");
    assert_eq!(
        p,
        PathBuf::from(format!("/home/u/.aenv/cache/skills/{hash}/head"))
    );
}
