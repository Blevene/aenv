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
    json: bool,
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
    // Project-side: pre-flight resolves $HOME / $AENV_TARGET_ROOT against the
    // project root. Project-scope settings.json files (rare) tend to use
    // project-relative paths, so this is the right anchor.
    let report = aenv_core::doctor::evaluate(fs, layout, adapters, &resolved, project_root);

    if json {
        let report_json = aenv_core::json::DoctorReportJson::from_report(&leaf_name, &report);
        println!(
            "{}",
            serde_json::to_string_pretty(&report_json)
                .map_err(|e| AenvError::ManifestInvalid(format!("json: {e}")))?
        );
    } else {
        print_report(&leaf_name, &report);
    }

    if report.has_enforce_violations() {
        return Err(AenvError::PolicyViolation(report.summary_line()));
    }

    Ok(())
}

/// Print the doctor report to stdout.
///
/// Format follows functional spec §5.12:
/// - Header: `Namespace 'X' (resolution: a → b)`
/// - Active policies block with `(from <ns>[, inherited])` suffix
/// - `Issues:` section listing every Fail/Warn as `✗ POLICY violation: <key>`
///   with `skill:` / `file:` / `hint:` sub-labels
/// - `Skipped:` section for unknown keys
/// - Footer summary line
fn print_report(leaf: &str, report: &aenv_core::doctor::DoctorReport) {
    // Header line.
    let chain_str: Vec<&str> = report
        .chain
        .iter()
        .map(aenv_core::identity::NamespaceId::as_str)
        .collect();
    println!("Namespace '{leaf}' (resolution: {})", chain_str.join(" → "));
    println!();

    // Active policies section. The `(from <ns>, inherited)` suffix matches
    // spec §5.12: a policy whose source is an ancestor of the leaf is shown
    // as inherited; one declared by the leaf itself omits the suffix.
    if report.policies.is_empty() {
        println!("Active policies: (none)");
    } else {
        println!("Active policies (after inheritance):");
        for (key, rp) in &report.policies {
            let enforce_str = if rp.enforce { " enforce=true" } else { "" };
            let inherited = rp.source.as_str() != leaf;
            let source_suffix = if inherited {
                format!("(from {}, inherited)", rp.source)
            } else {
                format!("(from {})", rp.source)
            };
            println!(
                "  {key:30} = {} {source_suffix}{enforce_str}",
                rp.value_display(),
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

    // Per spec §5.12, both enforced and advisory violations render under a
    // single `Issues:` header with the same `✗` marker. The summary line at
    // the bottom is what distinguishes the two flavors.
    if !fails.is_empty() || !warns.is_empty() {
        println!("Issues:");
        for o in fails.iter().chain(warns.iter()) {
            let msg = match &o.status {
                OutcomeStatus::Fail { msg } | OutcomeStatus::Warn { msg } => msg.as_str(),
                _ => continue,
            };
            println!("  ✗ POLICY violation: {}", o.key);
            print_issue_target(&o.target);
            print_hint(msg);
            println!();
        }
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

/// Print `skill:` and/or `file:` sub-labels for an issue's target.
///
/// A target whose short name matches `.claude/skills/<dir>/SKILL.md` is shown
/// as a `skill:` qualified by the namespace and the skill directory name,
/// plus a separate `file:` line for the on-disk path. Anything else gets a
/// bare `file:` line.
fn print_issue_target(target: &Option<aenv_core::identity::QualifiedName>) {
    let Some(qn) = target else {
        return;
    };
    let short = qn.short().as_str();
    let parts: Vec<&str> = short.split('/').collect();
    if parts.len() == 4 && parts[0] == ".claude" && parts[1] == "skills" && parts[3] == "SKILL.md" {
        println!("    skill:   {}::{}", qn.namespace(), parts[2]);
        println!("    file:    {short}");
    } else {
        println!("    file:    {short}");
    }
}

/// Print `hint:` on the first line of `msg` and continuation indentation
/// on subsequent lines. Spec §5.12 shows hints wrapped at ~70 cols; we keep
/// the wrapping the message-author chose (single-line for our four built-ins)
/// rather than re-wrapping here.
fn print_hint(msg: &str) {
    for (i, line) in msg.lines().enumerate() {
        if i == 0 {
            println!("    hint:    {line}");
        } else {
            println!("             {line}");
        }
    }
}
