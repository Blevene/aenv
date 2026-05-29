//! `aenv global activate <ns>` — activate a namespace globally.
//!
//! When the namespace declares `[lifecycle].on_activate`, the user must
//! approve the script before it runs. The approval is namespace-scoped
//! and SHA-pinned: editing the script invalidates the prior approval and
//! triggers a re-prompt. `--yes` records approval without prompting.
//!
//! Before the lifecycle step, a pre-flight scan walks every settings.json
//! candidate and reports hook / MCP / statusLine command paths that point
//! at files that don't exist on disk and aren't being materialized by
//! this activation. If any findings, the user is prompted to continue.
//! `--yes` reports the findings and proceeds without prompting.

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use std::io::{BufRead, Write};
use std::path::Path;

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    fake_home: &Path,
    name: &str,
    yes: bool,
) -> Result<()> {
    let leaf = NamespaceId::new(name).map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;

    // Pre-flight: scan settings.json candidates BEFORE prompting for the
    // lifecycle script. A missing hook target is the F5 lockout class
    // — surfacing it up front is cheaper than discovering it after the
    // activation succeeds and hooks deny every subsequent shell call.
    {
        let resolution = aenv_core::resolve::resolve_namespace(fs, layout, adapters, &leaf)?;
        let findings = aenv_core::preflight::preflight_settings_commands(
            fs,
            fake_home,
            &resolution.candidates,
        )?;
        if !findings.is_empty() {
            let n = findings.len();
            eprintln!(
                "Pre-flight found {n} potential issue{}:",
                if n == 1 { "" } else { "s" }
            );
            for f in &findings {
                eprintln!(
                    "  - {} in {}: command '{}' references {} (missing)",
                    f.kind.as_label(),
                    f.settings_path.display(),
                    f.command,
                    f.missing_path.display(),
                );
            }
            if yes {
                eprintln!("Continuing because --yes was passed.");
            } else {
                eprintln!();
                eprint!("Continue activation despite missing paths? [y/N]: ");
                std::io::stderr().flush().map_err(AenvError::Io)?;
                let stdin = std::io::stdin();
                let mut line = String::new();
                stdin.lock().read_line(&mut line).map_err(AenvError::Io)?;
                let answer = line.trim();
                if !answer.eq_ignore_ascii_case("y") && !answer.eq_ignore_ascii_case("yes") {
                    println!("Aborted: pre-flight not approved.");
                    return Ok(());
                }
            }
        }
    }

    // Inspect the manifest for an `on_activate` hook before doing any work
    // so we can prompt up front. A namespace-scoped, sha-pinned `.approved`
    // marker records prior consent; we only prompt the user when the
    // marker is missing or the script has changed.
    let manifest = aenv_core::activate::load_leaf_manifest(fs, layout, &leaf)?;
    if let Some(script_rel) = manifest.lifecycle.on_activate.as_deref() {
        let script_path = layout.namespace_dir(name).join(script_rel);
        // If the script is declared but missing, let the activator surface
        // the canonical error (`ManifestInvalid`) — duplicating the check
        // here would just diverge over time.
        if fs.exists(&script_path)? {
            let status = super::approval::current_status(layout, &leaf, Some(&script_path))?;
            match status {
                super::approval::ApprovalStatus::Approved
                | super::approval::ApprovalStatus::NoScript => { /* proceed silently */ }
                super::approval::ApprovalStatus::NotApproved { current_sha } => {
                    if yes {
                        super::approval::record_approval(layout, &leaf, &current_sha)?;
                    } else {
                        let ok = super::approval::prompt_user(&script_path, &current_sha, None)?;
                        if !ok {
                            println!("Aborted: lifecycle script not approved.");
                            return Ok(());
                        }
                        super::approval::record_approval(layout, &leaf, &current_sha)?;
                    }
                }
                super::approval::ApprovalStatus::ScriptChanged {
                    previous_sha,
                    current_sha,
                } => {
                    if yes {
                        super::approval::record_approval(layout, &leaf, &current_sha)?;
                    } else {
                        let ok = super::approval::prompt_user(
                            &script_path,
                            &current_sha,
                            Some(&previous_sha),
                        )?;
                        if !ok {
                            println!("Aborted: script change not re-approved.");
                            return Ok(());
                        }
                        super::approval::record_approval(layout, &leaf, &current_sha)?;
                    }
                }
            }
        }
    }

    let state = aenv_core::activate::swap_or_activate_user(fs, layout, adapters, fake_home, &leaf)?;
    for w in &state.warnings {
        eprintln!("[aenv] warning: {w}");
    }
    println!(
        "Activated '{}' globally in {}",
        state.active_namespace,
        fake_home.display()
    );
    for file in &state.managed_files {
        println!("  + {} ({:?})", file.path.display(), file.strategy);
    }
    if !state.backed_up.is_empty() {
        println!("Backed up {} file(s):", state.backed_up.len());
        for backup in &state.backed_up {
            println!(
                "  - {} -> {}",
                backup.original_path.display(),
                backup.backup_path.display()
            );
        }
    }
    println!("Note: running harness sessions retain their previous config until restart.");
    Ok(())
}
