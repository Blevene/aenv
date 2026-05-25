//! `aenv activate-if-needed [--last-active <path>]` — the fast-path the
//! shell hook calls on every chpwd. Walks the cwd's ancestors for a
//! `.aenv` pin and transitions the project's active namespace when it
//! differs from what the previous hook invocation activated.
//!
//! State protocol: the caller passes the previous return value via
//! `--last-active`; this command prints the new active project root to
//! stdout (or an empty string if no `.aenv` pin is in scope). The shell
//! hook captures stdout and stores it in `_AENV_ACTIVE`.
//!
//! Performance contract: the no-change path must stay sub-10 ms. We avoid
//! reading or parsing `state.json` on that path — the (last_active, new)
//! equality check plus an ancestor walk is the entire critical section.

use aenv_core::activate::activate_namespace;
use aenv_core::adapter::AdapterRegistry;
use aenv_core::deactivate::deactivate_namespace;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::project::{find_project_root, read_pin};
use aenv_core::{AenvError, Result};
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    last_active: Option<&Path>,
) -> Result<()> {
    let cwd = std::env::current_dir().map_err(AenvError::Io)?;
    let desired = find_project_root(fs, &cwd).ok();

    let prev = last_active.filter(|p| !p.as_os_str().is_empty());

    match (prev, desired.as_ref()) {
        // No previous, no current. Nothing to do.
        (None, None) => {}

        // Same project as last time. Fast path: trust that the previous
        // invocation left state.json correct. The user shouldn't be
        // mixing manual `aenv use` with the hook anyway.
        (Some(p), Some(c)) if p == c.as_path() => {}

        // Different project (or first time entering one). Deactivate
        // whatever was active before, then activate the new pin.
        (prev_opt, Some(c)) => {
            if let Some(p) = prev_opt {
                deactivate_if_state_present(fs, p)?;
            }
            activate(fs, layout, c)?;
        }

        // Left every aenv-pinned scope. Deactivate the previous project.
        (Some(p), None) => {
            deactivate_if_state_present(fs, p)?;
        }
    }

    print_path(desired.as_deref());
    Ok(())
}

fn deactivate_if_state_present<F: Filesystem>(fs: &F, project: &Path) -> Result<()> {
    let state_path = project.join(".aenv-state/state.json");
    if fs.exists(&state_path)? {
        let _ = deactivate_namespace(fs, project);
    }
    Ok(())
}

fn activate<F: Filesystem>(fs: &F, layout: &RegistryLayout, project: &Path) -> Result<()> {
    let pin = read_pin(fs, project)?;
    let leaf = NamespaceId::new(pin.clone())
        .map_err(|e| AenvError::ManifestInvalid(format!("namespace id: {e}")))?;
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let _ = activate_namespace(fs, layout, &adapters, project, &leaf)?;
    Ok(())
}

fn print_path(path: Option<&Path>) {
    match path {
        Some(p) => println!("{}", p.display()),
        None => println!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aenv_core::fs::MockFilesystem;
    use std::path::PathBuf;

    #[test]
    fn empty_string_last_active_treated_as_none() {
        // The shell hook passes ${_AENV_ACTIVE:-} which is the empty
        // string before the var has ever been set. Treat that as "no
        // previous activation," not as a path equal to "".
        let p = Path::new("");
        let _filtered: Option<&Path> = Some(p).filter(|q| !q.as_os_str().is_empty());
        assert!(_filtered.is_none());
    }

    #[test]
    fn deactivate_if_state_present_is_noop_when_missing() {
        let fs = MockFilesystem::new();
        let project = PathBuf::from("/projects/p");
        fs.create_dir_all(&project).unwrap();
        // No .aenv-state/state.json — should not error.
        assert!(deactivate_if_state_present(&fs, &project).is_ok());
    }
}
