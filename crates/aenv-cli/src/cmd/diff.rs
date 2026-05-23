//! aenv diff: project drift (no args) and structural (two-namespace).

use aenv_core::adapter::AdapterRegistry;
use aenv_core::diff::{project_drift, structural};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::{AenvError, Result};
use std::path::Path;

/// Run `aenv diff` (no positional args) — report project drift.
pub fn run_drift<F: Filesystem>(
    fs: &F,
    project_root: &Path,
    aenv_home: &Path,
    json: bool,
) -> Result<()> {
    let layout = RegistryLayout::new(aenv_home.to_path_buf());
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let report = project_drift(fs, &layout, &adapters, project_root)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| AenvError::ManifestInvalid(format!("json: {e}")))?
        );
    } else if report.drifted.is_empty() {
        println!("No drift detected. All managed files match their namespace source.");
    } else {
        println!("Drift in project {}:", report.project.display());
        for d in &report.drifted {
            println!("  {} ({})", d.path.display(), d.kind);
            if let Some(s) = &d.summary {
                println!("    {s}");
            }
        }
    }
    Ok(())
}

/// Run `aenv diff <a> <b>` — report structural difference between two namespaces.
pub fn run_structural<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    a: &str,
    b: &str,
    json: bool,
) -> Result<()> {
    let adapters = AdapterRegistry::load_from_dir(fs, &layout.adapters_dir())?;
    let diff = structural(fs, layout, &adapters, a, b)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&diff)
                .map_err(|e| AenvError::ManifestInvalid(format!("json: {e}")))?
        );
        return Ok(());
    }

    if !diff.skills.added.is_empty() || !diff.skills.removed.is_empty() {
        println!("Skills:");
        for s in &diff.skills.added {
            println!("  + {s}");
        }
        for s in &diff.skills.removed {
            println!("  - {s}");
        }
        println!();
    }
    if !diff.parameters.added.is_empty()
        || !diff.parameters.removed.is_empty()
        || !diff.parameters.changed.is_empty()
    {
        println!("Parameters:");
        for c in &diff.parameters.changed {
            println!("  {}: {} → {}", c.name, c.a, c.b);
        }
        for nv in &diff.parameters.added {
            println!("  +{}: {}", nv.name, nv.value);
        }
        for nv in &diff.parameters.removed {
            println!("  -{}: {}", nv.name, nv.value);
        }
        println!();
    }
    if !diff.policies.added.is_empty()
        || !diff.policies.removed.is_empty()
        || !diff.policies.changed.is_empty()
    {
        println!("Policies:");
        for c in &diff.policies.changed {
            println!("  {}: {} → {}", c.name, c.a, c.b);
        }
        for nv in &diff.policies.added {
            println!("  +{}: {}", nv.name, nv.value);
        }
        for nv in &diff.policies.removed {
            println!("  -{}: {}", nv.name, nv.value);
        }
        println!();
    }
    if !diff.instructions_sections.added.is_empty()
        || !diff.instructions_sections.removed.is_empty()
    {
        println!("Instructions sections:");
        for s in &diff.instructions_sections.added {
            println!("  + ## {s}");
        }
        for s in &diff.instructions_sections.removed {
            println!("  - ## {s}");
        }
    }
    Ok(())
}
