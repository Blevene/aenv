//! `aenv status [--project <path>]`.

use aenv_core::fs::Filesystem;
use aenv_core::identity::NamespaceId;
use aenv_core::resolve::{DeepMergeFormat, MaterializeStrategy};
use aenv_core::state::{ActivationState, ManagedFile};
use aenv_core::Result;
use std::path::Path;

/// Format activation state with resolution chain into human-readable output.
///
/// Shows the active namespace, resolution chain (root → leaf), and per-file
/// provenance (qualified name, merge strategy, contributors, shadows).
pub fn format_status(state: &ActivationState, chain: &[NamespaceId]) -> String {
    let mut out = String::new();
    out.push_str(&format!("Active namespace: {}\n", state.active_namespace));
    out.push_str("Resolution:       ");
    let rendered: Vec<&str> = chain.iter().map(|n| n.as_str()).collect();
    out.push_str(&rendered.join(" → "));
    out.push('\n');
    out.push('\n');

    if state.managed_files.is_empty() {
        out.push_str("No managed files.\n");
    } else {
        out.push_str("Managed files:\n");
        for mf in &state.managed_files {
            out.push_str(&format!("  ./{}\n", mf.path.display()));
            out.push_str(&format!("      {}\n", describe(mf)));
            for s in &mf.shadows {
                out.push_str(&format!("      (shadows {s})\n"));
            }
        }
    }

    if !state.backed_up.is_empty() {
        out.push('\n');
        out.push_str("Backed-up originals:\n");
        for b in &state.backed_up {
            out.push_str(&format!(
                "  {} -> {}\n",
                b.original_path.display(),
                b.backup_path.display()
            ));
        }
    }

    if !state.parameters.is_empty() {
        out.push('\n');
        out.push_str("Parameters:\n");
        for (k, rp) in &state.parameters {
            out.push_str(&format!("  {k:30} = {} (from {})\n", rp.value, rp.source));
        }
    }

    if !state.policies.is_empty() {
        out.push('\n');
        out.push_str("Active policies:\n");
        for (k, rp) in &state.policies {
            let enforce = if rp.enforce { " enforce=true" } else { "" };
            out.push_str(&format!(
                "  {k:30} = {} (from {}){}\n",
                rp.value_display(),
                rp.source,
                enforce
            ));
        }
    }

    // Skills section: group managed files by skill_provenance (only SKILL.md
    // files carry provenance, per Task 9's gather_skill_candidates).
    let skill_files: Vec<&ManagedFile> = state
        .managed_files
        .iter()
        .filter(|m| m.skill_provenance.is_some() && m.path.file_name() == Some("SKILL.md".as_ref()))
        .collect();
    if !skill_files.is_empty() {
        let authored_count = skill_files
            .iter()
            .filter(|m| {
                m.skill_provenance
                    .as_ref()
                    .map(|p| p.source.starts_with("authored:"))
                    .unwrap_or(false)
            })
            .count();
        let imported_count = skill_files.len() - authored_count;
        out.push('\n');
        out.push_str(&format!(
            "Skills ({authored_count} authored, {imported_count} imported):\n"
        ));
        for m in &skill_files {
            let prov = m.skill_provenance.as_ref().unwrap();
            let (mode, source) = if prov.source.starts_with("authored:") {
                ("authored", "-".to_string())
            } else {
                ("imported", prov.source.clone())
            };
            let ref_part = prov
                .resolved_ref
                .as_ref()
                .map(|r| format!(" @ {r}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {}  {mode}  {source}{ref_part}\n",
                m.qualified_name
            ));
        }
    }

    out
}

fn describe(mf: &ManagedFile) -> String {
    match mf.strategy {
        MaterializeStrategy::Symlink => format!("from {}", mf.qualified_name),
        MaterializeStrategy::Identical => {
            format!("identical to {} (no symlink)", mf.qualified_name)
        }
        MaterializeStrategy::Copy => format!("copy of {}", mf.qualified_name),
        MaterializeStrategy::SectionMerge => {
            let parts: Vec<String> = mf
                .contributors
                .iter()
                .map(|c| c.namespace().as_str().to_string())
                .collect();
            format!("merged from {}", parts.join(" + "))
        }
        MaterializeStrategy::DeepMerge(fmt) => {
            let parts: Vec<String> = mf
                .contributors
                .iter()
                .map(|c| c.namespace().as_str().to_string())
                .collect();
            let fmt_name = match fmt {
                DeepMergeFormat::Json => "json",
                DeepMergeFormat::Yaml => "yaml",
                DeepMergeFormat::Toml => "toml",
            };
            format!(
                "merged (deep-merge {}) from {}",
                fmt_name,
                parts.join(" + ")
            )
        }
        MaterializeStrategy::Merged => format!("merged (Phase 1 legacy) {}", mf.qualified_name),
    }
}

pub fn run<F: Filesystem>(fs: &F, project_root: &Path, aenv_home: &Path) -> Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    if !fs.exists(&state_path)? {
        println!("No active namespace in {}", project_root.display());
        return Ok(());
    }
    let bytes = fs.read(&state_path)?;
    let text = String::from_utf8(bytes)
        .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("state.json: {e}")))?;
    let state = ActivationState::from_json(&text)?;

    // Re-resolve the chain
    let registry = aenv_core::home::RegistryLayout::new(aenv_home.to_path_buf());
    let adapters =
        aenv_core::adapter::AdapterRegistry::load_from_dir(fs, &registry.adapters_dir())?;
    let leaf = NamespaceId::new(state.active_namespace.as_str())?;
    let resolution = aenv_core::resolve::resolve_namespace(fs, &registry, &adapters, &leaf)?;

    print!("{}", format_status(&state, &resolution.chain));
    Ok(())
}
