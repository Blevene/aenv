//! `aenv fork` — detach a file or entire project from namespace management.

use std::path::PathBuf;

/// Detach a single managed file from namespace management.
///
/// Replaces a symlinked artifact with a regular copy of its bytes and
/// removes it from `state.managed_files`. Merged files are already regular
/// files on disk; forking them leaves the file untouched but stops aenv
/// from regenerating them on subsequent activations.
pub fn run_file(project_root: PathBuf, rel: PathBuf) -> aenv_core::Result<()> {
    aenv_core::activate::fork_file(&aenv_core::fs::RealFilesystem, &project_root, &rel)?;
    println!("Forked {}:", rel.display());
    println!("  - replaced symlink with a copy at ./{}", rel.display());
    println!("  - removed from namespace management for this project");
    println!("  - subsequent activations will not touch this file");
    Ok(())
}

/// Detach the entire project from namespace management.
///
/// Replaces every symlinked managed file with a regular copy, then removes
/// `.aenv-state/` so subsequent activations skip the project. The `.aenv`
/// pin is retained for human reference.
pub fn run_project_detach(project_root: PathBuf) -> aenv_core::Result<()> {
    aenv_core::activate::fork_project(&aenv_core::fs::RealFilesystem, &project_root)?;
    println!("Forked project (detached from namespace management):");
    println!("  - replaced every symlinked managed file with a regular copy");
    println!("  - removed .aenv-state/ (state + backups)");
    println!("  - .aenv pin retained for reference; re-pin to re-activate");
    Ok(())
}

/// Create a new namespace from the current project's managed files, then
/// update the project pin to point at the new namespace.
pub fn run_name(
    aenv_home: PathBuf,
    project_root: PathBuf,
    new_name: String,
) -> aenv_core::Result<()> {
    let registry = aenv_core::home::RegistryLayout::new(aenv_home);
    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
        &aenv_core::fs::RealFilesystem,
        &registry.adapters_dir(),
    )?;
    aenv_core::namespace::create_namespace_from_project(
        &aenv_core::fs::RealFilesystem,
        &registry,
        &adapters,
        &new_name,
        &project_root,
    )?;
    aenv_core::project::write_pin(&aenv_core::fs::RealFilesystem, &project_root, &new_name)?;
    println!("Forked project into new namespace '{new_name}'");
    println!("  - copied harness files into ~/.aenv/envs/{new_name}/");
    println!("  - updated .aenv pin");
    println!("  - run 'aenv activate' to materialize");
    Ok(())
}
