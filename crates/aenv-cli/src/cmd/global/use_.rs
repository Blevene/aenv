//! `aenv global use <target>` — the one-command front door for global profiles.
//!
//! `<target>` resolves, in precedence order, to:
//!   1. `-`           → the previously-active profile (toggle back).
//!   2. a git URL     → import it (if not already present), then activate.
//!   3. a known name  → switch the active global profile to it.
//!   4. a local dir   → import it (if not already present), then activate.
//!
//! Importing-then-activating collapses the old `snapshot` + `import` +
//! `activate` ritual into a single command. The previously-active namespace is
//! recorded so `aenv global use -` toggles back. Baseline capture and the
//! pre-flight / lifecycle-approval gates are inherited from `activate`.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use std::path::Path;

use super::import::{
    default_name_for, default_name_from_url, looks_like_git_url, resolve_local_source,
};

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    fake_home: &Path,
    target: &str,
    as_name: Option<&str>,
    pin: Option<&str>,
    yes: bool,
    no_baseline: bool,
) -> Result<()> {
    // Record what's active *before* this swap so we can offer `use -`.
    let prev_active = active_namespace(fs, layout)?;

    let ns_name = resolve_target(fs, layout, adapters, target, as_name, pin)?;

    super::activate::run(fs, layout, adapters, fake_home, &ns_name, yes, no_baseline)?;

    // Persist the previous profile for `use -`, unless we just re-activated the
    // same one (toggling to yourself is a no-op we don't want to record).
    if let Some(prev) = prev_active {
        if prev != ns_name {
            fs.write(&layout.global_previous_path(), prev.as_bytes())?;
        }
    }
    Ok(())
}

/// Resolve `<target>` to a concrete namespace name, importing first when the
/// target is a source that hasn't been imported yet.
fn resolve_target<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    target: &str,
    as_name: Option<&str>,
    pin: Option<&str>,
) -> Result<String> {
    // 1. `-` → previous profile.
    if target == "-" {
        let prev_path = layout.global_previous_path();
        if !fs.exists(&prev_path)? {
            return Err(AenvError::ActivationConflict(
                "no previous global profile recorded; activate one by name first".into(),
            ));
        }
        let name = String::from_utf8(fs.read(&prev_path)?)
            .map_err(|e| AenvError::ManifestInvalid(format!("global-previous: {e}")))?
            .trim()
            .to_string();
        return Ok(name);
    }

    // 2. git URL → import (unless already imported under the resolved name).
    if looks_like_git_url(target) {
        let name = match as_name {
            Some(n) => n.to_string(),
            None => default_name_from_url(target)?,
        };
        if !namespace_exists(fs, layout, &name) {
            super::import::run(fs, layout, adapters, target, &name, pin)?;
        } else {
            println!("Namespace '{name}' already imported; switching to it.");
        }
        return Ok(name);
    }

    if pin.is_some() {
        return Err(AenvError::ManifestInvalid(
            "--pin only applies to git URL sources".into(),
        ));
    }

    // 3. An existing namespace name wins over a coincidental local path.
    if namespace_exists(fs, layout, target) {
        return Ok(target.to_string());
    }

    // 4. A local directory → import (unless already imported under the name).
    let src = resolve_local_source(target)?;
    if fs.exists(&src)? {
        let name = match as_name {
            Some(n) => n.to_string(),
            None => default_name_for(&src)?,
        };
        if !namespace_exists(fs, layout, &name) {
            super::import::run(fs, layout, adapters, target, &name, None)?;
        } else {
            println!("Namespace '{name}' already imported; switching to it.");
        }
        return Ok(name);
    }

    // 5. Nothing matched.
    Err(AenvError::NamespaceNotFound(target.to_string()))
}

/// Whether a namespace with this name exists in the registry.
fn namespace_exists<F: Filesystem>(fs: &F, layout: &RegistryLayout, name: &str) -> bool {
    fs.exists(&layout.manifest_path(name)).unwrap_or(false)
}

/// Read the currently-active global namespace name from `global-state.json`,
/// or `None` when no activation is live.
fn active_namespace<F: Filesystem>(fs: &F, layout: &RegistryLayout) -> Result<Option<String>> {
    let path = layout.global_state_path();
    if !fs.exists(&path)? {
        return Ok(None);
    }
    let bytes = fs.read(&path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("global-state.json: {e}")))?;
    let state = aenv_core::state::ActivationState::from_json(text)?;
    Ok(Some(state.active_namespace))
}
