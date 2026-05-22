//! `aenv doctor [<namespace>]` — evaluate policies for a namespace and report.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::OutcomeStatus;
use aenv_core::resolve::resolve_namespace;
use aenv_core::state::ActivationState;
use std::path::Path;

/// Run `aenv doctor [<namespace>]`.
///
/// If `ns_arg` is `Some(name)`, use that namespace as the leaf.
/// If `None`, read the active namespace from `<project_root>/.aenv-state/state.json`.
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    project_root: &Path,
    ns_arg: Option<&str>,
) -> Result<()> {
    // Determine the leaf namespace name.
    let leaf_name: String = match ns_arg {
        Some(name) => name.to_string(),
        None => {
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

    let leaf = NamespaceId::new(&leaf_name)?;
    let resolved = resolve_namespace(fs, layout, adapters, &leaf)?;
    let report = aenv_core::doctor::evaluate(fs, layout, adapters, &resolved);

    print_report(&leaf_name, &report);

    if report.has_enforce_violations() {
        return Err(AenvError::PolicyViolation(report.summary_line()));
    }

    Ok(())
}

/// Print the doctor report to stdout.
fn print_report(leaf: &str, report: &aenv_core::doctor::DoctorReport) {
    // Header line.
    let chain_str: Vec<&str> = report.chain.iter().map(|n| n.as_str()).collect();
    println!("Namespace '{leaf}' (resolution: {})", chain_str.join(" → "));
    println!();

    // Active policies section.
    if report.policies.is_empty() {
        println!("Active policies: (none)");
    } else {
        println!("Active policies (after inheritance):");
        for (key, rp) in &report.policies {
            let enforce_str = if rp.enforce { " enforce=true" } else { "" };
            println!(
                "  {key:30} = {} (from {}){enforce_str}",
                rp.value_display(),
                rp.source
            );
        }
    }
    println!();

    // Categorize outcomes.
    let fails: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Fail { .. }))
        .collect();
    let warns: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Warn { .. }))
        .collect();
    let warn_skips: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::WarnSkip { .. }))
        .collect();
    let pass_count = report
        .outcomes
        .iter()
        .filter(|o| matches!(o.status, OutcomeStatus::Pass))
        .count();

    if !fails.is_empty() {
        println!("Issues:");
        for o in &fails {
            println!("  X POLICY violation: {}", o.key);
            if let Some(target) = &o.target {
                println!("    target: {target}");
            }
            if let OutcomeStatus::Fail { msg } = &o.status {
                for line in msg.lines() {
                    println!("    {line}");
                }
            }
        }
        println!();
    }

    if !warns.is_empty() {
        println!("Advisory:");
        for o in &warns {
            println!("  ! POLICY {}", o.key);
            if let Some(target) = &o.target {
                println!("    target: {target}");
            }
            if let OutcomeStatus::Warn { msg } = &o.status {
                for line in msg.lines() {
                    println!("    {line}");
                }
            }
        }
        println!();
    }

    if !warn_skips.is_empty() {
        println!("Skipped:");
        for o in &warn_skips {
            if let OutcomeStatus::WarnSkip { msg } = &o.status {
                println!("  - {} ({msg})", o.key);
            }
        }
        println!();
    }

    // Footer.
    println!(
        "{pass_count} pass, {} warn, {} fail, {} skipped.",
        warns.len(),
        fails.len(),
        warn_skips.len()
    );
    println!("{}", report.summary_line());
}
