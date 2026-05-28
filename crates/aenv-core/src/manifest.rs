//! Namespace manifest (`aenv.toml`) parsing.
//!
//! Phase 3 adds `[parameters]` and (Task 6) `[policies]`. Both tables go
//! through a two-stage parse: first into `toml::Value`, then each entry is
//! validated and converted into its typed shape. Type errors surface as
//! `ManifestInvalid` (exit 12).

use crate::error::{AenvError, Result};
use crate::parameters::ParameterValue;
use crate::policies::{policy_table_from_toml, PolicyDecl};
use crate::skills::SkillDecl;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A parsed namespace manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AenvManifest {
    /// Namespace name (must match the directory name; checked at activation time).
    pub name: String,

    /// Parent namespaces to inherit from. Resolution lives in Phase 2's
    /// `resolve::resolve_namespace`.
    #[serde(default)]
    pub extends: Vec<String>,

    /// Per-adapter configuration. Keys are adapter names (e.g. "claude-code").
    #[serde(default)]
    pub adapters: BTreeMap<String, AdapterEntry>,

    /// Typed parameters. Always non-`None` after a successful `from_toml`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterValue>,

    /// Policy declarations. Keys are policy names; values hold the typed value
    /// and whether the policy is enforced.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub policies: BTreeMap<String, PolicyDecl>,

    /// Skill declarations from `[[skills]]` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillDecl>,

    /// Lifecycle scripts (on_activate / on_deactivate). Empty by default.
    #[serde(default, skip_serializing_if = "LifecycleHooks::is_empty")]
    pub lifecycle: LifecycleHooks,
}

/// Namespace lifecycle scripts. Paths are namespace-relative — `aenv`
/// resolves them against the namespace directory (e.g. `install.sh`
/// here means `<aenv_home>/envs/<ns>/install.sh`). Scripts run at the
/// boundaries defined in `pm_docs/lifecycle-hooks.md` (Task 11).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleHooks {
    /// Script to run AFTER files are materialized but BEFORE the state
    /// file is written. Non-zero exit rolls back the materialization.
    /// Path is namespace-relative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_activate: Option<String>,
    /// Script to run BEFORE files are undone during deactivation. Non-zero
    /// exit logs a warning but does not block deactivation (the user is
    /// likely recovering from a broken state). Path is namespace-relative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_deactivate: Option<String>,
}

impl LifecycleHooks {
    /// Returns true if both hooks are unset — used by `skip_serializing_if`
    /// so the default form emits no `[lifecycle]` block.
    pub fn is_empty(&self) -> bool {
        self.on_activate.is_none() && self.on_deactivate.is_none()
    }
}

/// Per-adapter manifest entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterEntry {
    /// Project-relative paths the adapter manages.
    #[serde(default)]
    pub files: Vec<String>,
    /// Per-file merge override. Key is relative path; value is one of:
    /// "section", "deep", "last-wins", "symlink".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<std::collections::BTreeMap<String, String>>,
    /// User-scope analog of `files`. Paths are relative to the namespace's
    /// `user/` source subdir, and to `$HOME` at activation time.
    ///
    /// Namespaces MAY declare paths that the adapter's own `user_files`
    /// doesn't list — this lets per-namespace harnesses extend the surface
    /// (e.g. `claude-cntrl` adds `.claude/runtime/` and `.claude/bin/` even
    /// though the builtin claude-code adapter doesn't). No containment check
    /// is enforced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_files: Vec<String>,
    /// User-scope analog of `merge`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_merge: Option<std::collections::BTreeMap<String, String>>,
    /// Override the adapter's default materialize strategy for this
    /// namespace. Same values as `Adapter::materialize` (`"symlink"` or
    /// `"copy"`). Per-file `merge` overrides still take precedence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialize: Option<String>,
}

impl AenvManifest {
    /// Parse a manifest from a TOML string. Two-stage: serde does the
    /// structural parse; then `[parameters]` entries are validated via
    /// `ParameterValue::from_toml_value`.
    pub fn from_toml(input: &str) -> Result<Self> {
        // `merge` on an adapter block accepts either a bare string
        // (`merge = "deep"`) or a per-file map (`merge = { "f.json" = "deep" }`).
        // The bare form means "apply this strategy to every file in `files`."
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum MergeRaw {
            /// `merge = "deep"` — uniform strategy for all files.
            Uniform(String),
            /// `merge = { ".mcp.json" = "deep" }` — per-file strategies.
            PerFile(BTreeMap<String, String>),
        }

        /// Raw adapter entry used only during parsing; accepts both merge forms.
        #[derive(Deserialize)]
        struct RawAdapterEntry {
            #[serde(default)]
            files: Vec<String>,
            #[serde(default)]
            merge: Option<MergeRaw>,
            #[serde(default)]
            user_files: Vec<String>,
            #[serde(default)]
            user_merge: Option<MergeRaw>,
            #[serde(default)]
            materialize: Option<String>,
        }

        // Stage 1: structural parse into a raw shape that holds parameters as
        // `toml::Value` so we can validate per-entry.
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            #[serde(default)]
            extends: Vec<String>,
            #[serde(default)]
            adapters: BTreeMap<String, RawAdapterEntry>,
            #[serde(default)]
            parameters: BTreeMap<String, toml::Value>,
            #[serde(default)]
            policies: BTreeMap<String, toml::Value>,
            #[serde(default)]
            skills: Vec<SkillDecl>,
            #[serde(default)]
            lifecycle: LifecycleHooks,
        }
        let raw: Raw =
            toml::from_str(input).map_err(|e| AenvError::ManifestInvalid(format!("{e}")))?;

        // Stage 2: expand adapter merge fields — bare strings become per-file maps.
        let adapters: BTreeMap<String, AdapterEntry> = raw
            .adapters
            .into_iter()
            .map(|(name, raw_entry)| {
                let RawAdapterEntry {
                    files,
                    merge: merge_raw,
                    user_files,
                    user_merge: user_merge_raw,
                    materialize,
                } = raw_entry;
                let merge = merge_raw.map(|m| match m {
                    MergeRaw::PerFile(map) => map,
                    MergeRaw::Uniform(strategy) => files
                        .iter()
                        .map(|f| (f.clone(), strategy.clone()))
                        .collect(),
                });
                let user_merge = user_merge_raw.map(|m| match m {
                    MergeRaw::PerFile(map) => map,
                    MergeRaw::Uniform(strategy) => user_files
                        .iter()
                        .map(|f| (f.clone(), strategy.clone()))
                        .collect(),
                });
                (
                    name,
                    AdapterEntry {
                        files,
                        merge,
                        user_files,
                        user_merge,
                        materialize,
                    },
                )
            })
            .collect();

        // Stage 3: validate each parameter entry.
        let mut parameters: BTreeMap<String, ParameterValue> = BTreeMap::new();
        for (k, v) in &raw.parameters {
            let pv = ParameterValue::from_toml_value(v).map_err(|e| match e {
                AenvError::ManifestInvalid(reason) => {
                    AenvError::ManifestInvalid(format!("parameter '{k}': {reason}"))
                }
                other => other,
            })?;
            parameters.insert(k.clone(), pv);
        }

        // Stage 4: validate each policy entry.
        let policies = policy_table_from_toml(&raw.policies)?;

        // Stage 5: validate skills (duplicates, mode/source coherence).
        validate_skills(&raw.skills)?;

        // Stage 6: validate lifecycle script paths — same rules as
        // `[[skills]].path` (no absolute, no `..`, no `~/` prefix).
        if let Some(p) = raw.lifecycle.on_activate.as_deref() {
            validate_relative_path("lifecycle.on_activate", p)?;
        }
        if let Some(p) = raw.lifecycle.on_deactivate.as_deref() {
            validate_relative_path("lifecycle.on_deactivate", p)?;
        }

        Ok(AenvManifest {
            name: raw.name,
            extends: raw.extends,
            adapters,
            parameters,
            policies,
            skills: raw.skills,
            lifecycle: raw.lifecycle,
        })
    }

    /// Render the manifest to a canonical TOML string.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("AenvManifest serialization is infallible")
    }

    /// Build the manifest `aenv create <name>` writes by default.
    pub fn default_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            extends: Vec::new(),
            adapters: BTreeMap::new(),
            parameters: BTreeMap::new(),
            policies: BTreeMap::new(),
            skills: Vec::new(),
            lifecycle: LifecycleHooks::default(),
        }
    }
}

fn validate_skills(skills: &[crate::skills::SkillDecl]) -> crate::error::Result<()> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for s in skills {
        if !seen.insert(s.name.as_str()) {
            return Err(crate::error::AenvError::ManifestInvalid(format!(
                "skill '{}' declared more than once",
                s.name
            )));
        }
        match s.mode {
            crate::skills::SkillMode::Authored => {
                if s.source.is_some() {
                    return Err(crate::error::AenvError::ManifestInvalid(format!(
                        "skill '{}' is authored but declares a source; \
                         remove `source` or change mode to 'imported'",
                        s.name
                    )));
                }
                if s.path.is_some() {
                    return Err(crate::error::AenvError::ManifestInvalid(format!(
                        "skill '{}' is authored but declares a path; \
                         path applies to imported skills only",
                        s.name
                    )));
                }
            }
            crate::skills::SkillMode::Imported => {
                if s.source.is_none() {
                    return Err(crate::error::AenvError::ManifestInvalid(format!(
                        "skill '{}' is imported but declares no source",
                        s.name
                    )));
                }
                if let Some(path) = s.path.as_deref() {
                    validate_relative_path(&format!("skill '{}'", s.name), path)?;
                }
            }
        }
    }
    Ok(())
}

/// Reject path traversal, absolute paths, and `~/` prefixes for any manifest
/// field that names a file/dir under a fixed root (skills, lifecycle scripts).
///
/// `field_label` names the offending field in the error message (e.g.
/// `"skill 'foo'"` or `"lifecycle.on_activate"`).
fn validate_relative_path(field_label: &str, path: &str) -> crate::error::Result<()> {
    use std::path::Component;
    if path.is_empty() {
        return Err(crate::error::AenvError::ManifestInvalid(format!(
            "{field_label} has empty path; omit the field or set a sub-directory"
        )));
    }
    // Reject `~/` (the shell does tilde expansion; manifests don't get that
    // luxury, and silently failing to expand would be worse than rejecting).
    if path.starts_with("~/") || path == "~" {
        return Err(crate::error::AenvError::ManifestInvalid(format!(
            "{field_label} path '{path}' may not start with '~'; \
             paths are relative to the namespace dir"
        )));
    }
    let parsed = std::path::Path::new(path);
    if parsed.is_absolute() {
        return Err(crate::error::AenvError::ManifestInvalid(format!(
            "{field_label} path '{path}' must be relative"
        )));
    }
    for component in parsed.components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(crate::error::AenvError::ManifestInvalid(format!(
                    "{field_label} path '{path}' may not contain '..' or other \
                     non-normal components"
                )));
            }
        }
    }
    Ok(())
}
