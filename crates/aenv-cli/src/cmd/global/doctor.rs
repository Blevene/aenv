//! `aenv global doctor [<ns>]` — user-scope policy evaluation.
//!
//! Resolves a namespace (either explicitly passed or read from
//! `global-state.json`), runs `aenv_core::doctor::evaluate`, and filters the
//! outcomes to those whose target label is a user-scope path (i.e. the
//! qualified-name display contains `::~/`).

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::identity::NamespaceId;
use aenv_core::policies::builtin::{OutcomeStatus, PolicyOutcome};

pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    fake_home: &std::path::Path,
    namespace: Option<&str>,
    json: bool,
) -> Result<()> {
    // Detect orphan stashes up-front: subdirs of `<aenv_home>/global-stash/`
    // not referenced by the active state. Surfaced informationally when a
    // specific namespace is being audited; treated as a hard error (exit 19)
    // when the user is auditing global state as a whole (no namespace arg).
    let orphans = aenv_core::state::list_orphan_stashes(layout)?;

    // Resolve which namespace to audit. If none was passed, read the active
    // global state. With no active state, we still need to report orphans
    // (and error out): fabricate a minimal report path that skips the policy
    // evaluation and goes straight to the orphan branch.
    let ns_name = match namespace {
        Some(n) => Some(n.to_string()),
        None => {
            let path = layout.global_state_path();
            if !fs.exists(&path)? {
                None
            } else {
                let bytes = fs.read(&path)?;
                let text = std::str::from_utf8(&bytes)
                    .map_err(|e| AenvError::ManifestInvalid(format!("global-state.json: {e}")))?;
                let state = aenv_core::state::ActivationState::from_json(text)?;
                Some(state.active_namespace)
            }
        }
    };

    // No namespace to evaluate and no active state: handle the two sub-cases.
    let Some(ns_name) = ns_name else {
        if json {
            let orphan_paths: Vec<String> =
                orphans.iter().map(|p| p.display().to_string()).collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "scope": "user",
                    "namespace": null,
                    "outcomes": [],
                    "orphan_stashes": orphan_paths,
                }))
                .unwrap()
            );
        } else if orphans.is_empty() {
            // Nothing to do — but the user asked for a global audit with no
            // activation. Match the previous behaviour: error out.
            return Err(AenvError::ActivationConflict(
                "no global activation; pass a namespace name to evaluate one directly".into(),
            ));
        } else {
            print_orphans_text(&orphans);
        }
        if !orphans.is_empty() {
            return Err(AenvError::GlobalConflict(format!(
                "{} orphan stash{} found; run `aenv global deactivate --prune` to clear",
                orphans.len(),
                if orphans.len() == 1 { "" } else { "es" },
            )));
        }
        return Ok(());
    };

    let leaf = NamespaceId::new(ns_name.as_str())
        .map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    let resolution = aenv_core::resolve::resolve_namespace(fs, layout, adapters, &leaf)?;
    // Pre-flight resolves $HOME / $AENV_TARGET_ROOT against `$HOME` for the
    // global-scope doctor invocation. `fake_home` is the test-overridable
    // alias for $HOME used throughout the global surface.
    let report = aenv_core::doctor::evaluate(fs, layout, adapters, &resolution, fake_home);

    // Filter to user-scope outcomes. The QualifiedName display form is
    // `<ns>::<short>`; user-scope ShortNames carry the `~/` prefix per the
    // Milestone B `target_label` convention, so the substring `::~/` is the
    // diagnostic marker. A bare `~/` prefix is accepted defensively.
    let user_outcomes: Vec<&PolicyOutcome> = report
        .outcomes
        .iter()
        .filter(|o| {
            let Some(t) = o.target.as_ref() else {
                return false;
            };
            let s = t.to_string();
            s.contains("::~/") || s.starts_with("~/")
        })
        .collect();

    if json {
        let outcomes_json: Vec<serde_json::Value> = user_outcomes
            .iter()
            .map(|o| {
                let status = match &o.status {
                    OutcomeStatus::Pass => "pass",
                    OutcomeStatus::Warn { .. } => "warn",
                    OutcomeStatus::Fail { .. } => "fail",
                    OutcomeStatus::WarnSkip { .. } => "warn-skip",
                };
                let msg = match &o.status {
                    OutcomeStatus::Pass => String::new(),
                    OutcomeStatus::Warn { msg }
                    | OutcomeStatus::Fail { msg }
                    | OutcomeStatus::WarnSkip { msg } => msg.clone(),
                };
                serde_json::json!({
                    "key": o.key,
                    "status": status,
                    "target": o.target.as_ref().map(std::string::ToString::to_string),
                    "msg": msg,
                })
            })
            .collect();
        let orphan_paths: Vec<String> = orphans.iter().map(|p| p.display().to_string()).collect();
        let payload = serde_json::json!({
            "scope": "user",
            "namespace": ns_name,
            "outcomes": outcomes_json,
            "orphan_stashes": orphan_paths,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else {
        if user_outcomes.is_empty() {
            println!("No user-scope issues found for '{ns_name}'.");
        } else {
            for o in &user_outcomes {
                let prefix = match &o.status {
                    OutcomeStatus::Pass => "[PASS]",
                    OutcomeStatus::Warn { .. } => "[WARN]",
                    OutcomeStatus::Fail { .. } => "[FAIL]",
                    OutcomeStatus::WarnSkip { .. } => "[SKIP]",
                };
                let target = o
                    .target
                    .as_ref()
                    .map(std::string::ToString::to_string)
                    .unwrap_or_default();
                let msg = match &o.status {
                    OutcomeStatus::Pass => String::new(),
                    OutcomeStatus::Warn { msg }
                    | OutcomeStatus::Fail { msg }
                    | OutcomeStatus::WarnSkip { msg } => format!(" — {msg}"),
                };
                println!("{prefix} {} {target}{msg}", o.key);
            }
        }
        if !orphans.is_empty() {
            print_orphans_text(&orphans);
        }
    }

    // Orphan-stash detection is itself an error condition when the user is
    // auditing global state as a whole (no namespace argument). When they
    // pass a namespace, they're inspecting that namespace specifically — just
    // report and exit 0.
    if namespace.is_none() && !orphans.is_empty() {
        return Err(AenvError::GlobalConflict(format!(
            "{} orphan stash{} found; run `aenv global deactivate --prune` to clear",
            orphans.len(),
            if orphans.len() == 1 { "" } else { "es" },
        )));
    }

    Ok(())
}

fn print_orphans_text(orphans: &[std::path::PathBuf]) {
    println!("Orphan stashes:");
    for p in orphans {
        println!("  {}", p.display());
    }
}
