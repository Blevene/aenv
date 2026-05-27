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
    namespace: Option<&str>,
    json: bool,
) -> Result<()> {
    // Resolve which namespace to audit. If none was passed, read the active
    // global state; if there is no active state, error out.
    let ns_name = match namespace {
        Some(n) => n.to_string(),
        None => {
            let path = layout.global_state_path();
            if !fs.exists(&path)? {
                return Err(AenvError::ActivationConflict(
                    "no global activation; pass a namespace name to evaluate one directly".into(),
                ));
            }
            let bytes = fs.read(&path)?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|e| AenvError::ManifestInvalid(format!("global-state.json: {e}")))?;
            let state = aenv_core::state::ActivationState::from_json(text)?;
            state.active_namespace
        }
    };

    let leaf = NamespaceId::new(ns_name.as_str())
        .map_err(|e| AenvError::ManifestInvalid(e.to_string()))?;
    let resolution = aenv_core::resolve::resolve_namespace(fs, layout, adapters, &leaf)?;
    let report = aenv_core::doctor::evaluate(fs, layout, adapters, &resolution);

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
        let payload = serde_json::json!({
            "scope": "user",
            "namespace": ns_name,
            "outcomes": outcomes_json,
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else if user_outcomes.is_empty() {
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

    Ok(())
}
