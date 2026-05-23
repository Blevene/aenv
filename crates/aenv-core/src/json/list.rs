//! Schema for `aenv list --json`. Matches functional spec §7.1.

use serde::Serialize;

/// JSON shape for one namespace row in `aenv list --json`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ListEntry {
    /// Fully-qualified namespace name.
    pub name: String,
    /// Namespaces this namespace extends, in declaration order.
    pub extends: Vec<String>,
    /// Adapters declared in this namespace.
    pub adapters: Vec<String>,
    /// Parameter keys declared directly (not inherited).
    pub parameters_declared: Vec<String>,
    /// Policy keys declared directly (not inherited).
    pub policies_declared: Vec<String>,
    /// `sha256-v1:<hex>` of the resolved namespace. Absent if resolution failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash: Option<String>,
    /// R-87 forward-compatibility hook (always None in v1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_hash_v2: Option<String>,
    /// If resolution failed, the error message lands here. The entry is
    /// still emitted so a script gets every namespace, not just the
    /// healthy ones.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

use crate::adapter::AdapterRegistry;
use crate::fs::Filesystem;
use crate::hash::hash_resolved_namespace;
use crate::home::RegistryLayout;
use crate::identity::NamespaceId;
use crate::manifest::AenvManifest;
use crate::materialize::compute_material_set;

impl ListEntry {
    /// Build a `ListEntry` for one namespace by reading its manifest
    /// and (best-effort) resolving + hashing it. Resolution errors are
    /// captured in `error`; the entry is still emitted so scripts get
    /// every namespace.
    pub fn build<F: Filesystem>(
        fs: &F,
        layout: &RegistryLayout,
        adapters: &AdapterRegistry,
        name: &str,
    ) -> Self {
        let manifest_path = layout.manifest_path(name);
        let manifest = match fs
            .read(&manifest_path)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| AenvManifest::from_toml(&s).ok())
        {
            Some(m) => m,
            None => {
                return ListEntry {
                    name: name.to_string(),
                    error: Some("manifest invalid or unreadable".into()),
                    ..Default::default()
                };
            }
        };

        let extends = manifest.extends.clone();
        let adapters_decl: Vec<String> = manifest.adapters.keys().cloned().collect();
        let parameters_declared: Vec<String> = manifest.parameters.keys().cloned().collect();
        let policies_declared: Vec<String> = manifest.policies.keys().cloned().collect();

        let leaf = match NamespaceId::new(name) {
            Ok(id) => id,
            Err(e) => {
                return ListEntry {
                    name: name.to_string(),
                    extends,
                    adapters: adapters_decl,
                    parameters_declared,
                    policies_declared,
                    error: Some(e.to_string()),
                    ..Default::default()
                };
            }
        };

        let (hash, error) = match compute_material_set(fs, layout, adapters, &leaf) {
            Ok(mat) => (Some(hash_resolved_namespace(&mat)), None),
            Err(e) => (None, Some(e.to_string())),
        };

        ListEntry {
            name: name.to_string(),
            extends,
            adapters: adapters_decl,
            parameters_declared,
            policies_declared,
            resolved_hash: hash,
            resolved_hash_v2: None,
            error,
        }
    }
}
