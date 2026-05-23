//! `aenv which <path>` — show which namespace manages a given file.

use std::path::{Path, PathBuf};

use aenv_core::resolve::{DeepMergeFormat, MaterializeStrategy};
use aenv_core::state::ActivationState;

/// Format a human-readable "which" report for `query` from `state`.
///
/// `aenv_home` is used to construct the `Source path:` line for strategies
/// where there is a single source file (Symlink, Copy, Identical). For merged
/// strategies (multiple contributors) the line is omitted.
///
/// Returns `Err(String)` if the path is not managed by the active namespace.
pub fn format_which(
    state: &ActivationState,
    query: &Path,
    aenv_home: &Path,
) -> Result<String, String> {
    let mf = state
        .managed_files
        .iter()
        .find(|m| m.path == query)
        .ok_or_else(|| {
            format!(
                "path {} is not managed by the active namespace",
                query.display()
            )
        })?;
    let mut out = String::new();
    out.push_str(&format!("Qualified name:  {}\n", mf.qualified_name));
    out.push_str(&format!("Materialized at: ./{}\n", query.display()));
    out.push_str(&format!(
        "Strategy:        {}\n",
        render_strategy(mf.strategy)
    ));
    // Source path: present only for single-source strategies (spec §5.5).
    match mf.strategy {
        MaterializeStrategy::Symlink
        | MaterializeStrategy::Copy
        | MaterializeStrategy::Identical => {
            let ns = mf.qualified_name.namespace().as_str();
            let short = mf.qualified_name.short().as_str();
            let src = aenv_home.join("envs").join(ns).join(short);
            out.push_str(&format!("Source path:     {}\n", src.display()));
        }
        _ => {}
    }
    if !mf.contributors.is_empty() {
        out.push_str("Contributors:    ");
        for (i, q) in mf.contributors.iter().enumerate() {
            if i > 0 {
                out.push_str("\n                 ");
            }
            out.push_str(&q.to_string());
        }
        out.push('\n');
    }
    if !mf.shadows.is_empty() {
        out.push_str("Shadows:         ");
        for (i, q) in mf.shadows.iter().enumerate() {
            if i > 0 {
                out.push_str("\n                 ");
            }
            out.push_str(&q.to_string());
        }
        out.push('\n');
    } else if mf.contributors.is_empty() {
        out.push_str("Shadows:         (nothing — no parent namespace defines this artifact)\n");
    }
    Ok(out)
}

fn render_strategy(s: MaterializeStrategy) -> String {
    match s {
        MaterializeStrategy::Symlink => "symlink".into(),
        MaterializeStrategy::Identical => "identical (project file already matches)".into(),
        MaterializeStrategy::Copy => "copy".into(),
        MaterializeStrategy::SectionMerge => "section-merge".into(),
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Json) => "deep-merge (json)".into(),
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Yaml) => "deep-merge (yaml)".into(),
        MaterializeStrategy::DeepMerge(DeepMergeFormat::Toml) => "deep-merge (toml)".into(),
        MaterializeStrategy::Merged => "merged (Phase 1 legacy)".into(),
    }
}

/// Entry point for `aenv which <path>`.
pub fn run(
    project_root: PathBuf,
    query: PathBuf,
    aenv_home: &Path,
    json: bool,
) -> aenv_core::Result<()> {
    let state_path = project_root.join(".aenv-state/state.json");
    let body = match std::fs::read(&state_path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(aenv_core::AenvError::ProjectNotPinned);
        }
        Err(e) => return Err(aenv_core::AenvError::from(e)),
    };
    let text = String::from_utf8(body)
        .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("state.json: {e}")))?;
    let state = ActivationState::from_json(&text)?;

    if json {
        let mf = state
            .managed_files
            .iter()
            .find(|m| m.path == query)
            .ok_or_else(|| {
                aenv_core::AenvError::ActivationConflict(format!(
                    "path {} is not managed by the active namespace",
                    query.display()
                ))
            })?;
        let report = aenv_core::json::WhichReport::from_managed_file(mf);
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| aenv_core::AenvError::ManifestInvalid(format!("json: {e}")))?
        );
        return Ok(());
    }

    let out = format_which(&state, &query, aenv_home)
        .map_err(aenv_core::AenvError::ActivationConflict)?;
    print!("{out}");
    Ok(())
}
