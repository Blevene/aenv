//! `aenv get <spec>` — print the effective value of a parameter plus provenance.
//!
//! `spec` is either `<namespace>.<parameter>` or `.<parameter>` (active project).

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::resolve::resolve_namespace;
use aenv_core::state::ActivationState;
use std::path::{Path, PathBuf};

/// Parse a `spec` string of the form `[<ns>].<param>`.
///
/// Returns `(Some(ns), param)` for `ns.param`, or `(None, param)` for `.param`.
/// Returns `ManifestInvalid` for missing `.`, empty param, or other malformed input.
fn parse_spec(spec: &str) -> Result<(Option<&str>, &str)> {
    match spec.find('.') {
        None => Err(AenvError::ManifestInvalid(format!(
            "spec '{spec}' has no '.'; expected '<namespace>.<param>' or '.<param>'"
        ))),
        Some(dot_pos) => {
            let param = &spec[dot_pos + 1..];
            if param.is_empty() {
                return Err(AenvError::ManifestInvalid(format!(
                    "spec '{spec}' has empty parameter name after '.'"
                )));
            }
            let ns = if dot_pos == 0 {
                None
            } else {
                let ns_str = &spec[..dot_pos];
                if ns_str.is_empty() {
                    return Err(AenvError::ManifestInvalid(format!(
                        "spec '{spec}' has empty namespace before '.'"
                    )));
                }
                Some(ns_str)
            };
            Ok((ns, param))
        }
    }
}

/// Run `aenv get <spec>`.
///
/// `project_root_hint` is only consulted when `spec` starts with `.` (active-project form).
/// For the explicit `<ns>.<param>` form it is ignored entirely, so the command works
/// outside any project directory.
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root_hint: Option<&Path>,
    spec: &str,
) -> Result<()> {
    let (ns_opt, param) = parse_spec(spec)?;

    // Resolve the leaf namespace name.
    let leaf_name: String = match ns_opt {
        Some(ns) => ns.to_string(),
        None => {
            // Resolve project root — required for the active-project (.<param>) form.
            let project_root: PathBuf = match project_root_hint {
                Some(p) => p.to_path_buf(),
                None => {
                    let cwd = std::env::current_dir().map_err(AenvError::Io)?;
                    aenv_core::project::find_project_root(fs, &cwd)?
                }
            };
            // Read active namespace from project state.
            let state_path = project_root.join(".aenv-state/state.json");
            if !fs.exists(&state_path)? {
                return Err(AenvError::ProjectNotPinned);
            }
            let bytes = fs.read(&state_path)?;
            let text = String::from_utf8(bytes).map_err(|e| {
                AenvError::ManifestInvalid(format!("state.json not valid UTF-8: {e}"))
            })?;
            let state = ActivationState::from_json(&text)?;
            state.active_namespace
        }
    };

    let leaf = aenv_core::identity::NamespaceId::new(&leaf_name)?;
    let rr = resolve_namespace(fs, layout, adapters, &leaf)?;

    // Look up the parameter in the resolution result.
    let rp = rr
        .parameters
        .get(param)
        .ok_or_else(|| AenvError::ParameterUndefined(format!("{leaf_name}.{param}")))?;

    // Line 1: value.
    println!("{}", rp.value);

    // Line 2: provenance.
    let source_str = rp.source.as_str();
    let chain = &rr.chain;

    // Find the position of source in the chain.
    let source_pos = chain.iter().position(|n| n.as_str() == source_str);

    // Find the latest namespace BEFORE source that also declared this param,
    // by re-reading each earlier manifest.
    let prior_info: Option<(String, String)> = if let Some(spos) = source_pos {
        let mut found: Option<(String, String)> = None;
        for ns in &chain[..spos] {
            let manifest_path = layout.manifest_path(ns.as_str());
            if let Ok(bytes) = fs.read(&manifest_path) {
                if let Ok(text) = String::from_utf8(bytes) {
                    if let Ok(manifest) = AenvManifest::from_toml(&text) {
                        if let Some(pv) = manifest.parameters.get(param) {
                            found = Some((ns.as_str().to_string(), pv.to_string()));
                        }
                    }
                }
            }
        }
        found
    } else {
        None
    };

    // Determine provenance message.
    let provenance = if source_str == leaf_name {
        // The leaf namespace supplied this value.
        match prior_info {
            Some((prior_ns, prior_val)) => {
                format!("  source: {source_str} (overrides {prior_ns} which declared {prior_val})")
            }
            None => format!("  source: {source_str} (declared, not inherited)"),
        }
    } else {
        // An ancestor namespace supplied this value; the leaf inherited it.
        format!("  source: {source_str} (inherited, not overridden)")
    };

    println!("{provenance}");
    Ok(())
}
