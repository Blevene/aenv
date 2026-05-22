//! `aenv set <namespace>.<parameter> <value>` — write a parameter into the
//! named namespace's manifest (PRD R-70). Value type is inferred from the
//! literal.

use aenv_core::error::AenvError;
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::parameters::ParameterValue;
use aenv_core::Result;

/// Entry point.
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    spec: &str,
    value_literal: &str,
) -> Result<()> {
    let (ns, param) = parse_spec(spec)?;
    let value = infer_value(value_literal);

    let manifest_path = layout.manifest_path(ns);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(ns.into()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("manifest not UTF-8: {e}")))?;
    let mut manifest = AenvManifest::from_toml(text)?;
    manifest.parameters.insert(param.to_string(), value);

    // Verify it still parses after the round-trip.
    let rendered = manifest.to_toml();
    let _ = AenvManifest::from_toml(&rendered)?;

    fs.write(&manifest_path, rendered.as_bytes())?;
    println!("Set {ns}.{param}");
    Ok(())
}

fn parse_spec(spec: &str) -> Result<(&str, &str)> {
    if spec.starts_with('.') {
        return Err(AenvError::ManifestInvalid(
            "'set' requires an explicit namespace: `aenv set <namespace>.<parameter> <value>`"
                .into(),
        ));
    }
    let (ns, param) = spec.split_once('.').ok_or_else(|| {
        AenvError::ManifestInvalid(format!("expected '<namespace>.<parameter>', got '{spec}'"))
    })?;
    if ns.is_empty() || param.is_empty() {
        return Err(AenvError::ManifestInvalid(format!(
            "invalid spec '{spec}': both namespace and parameter must be non-empty"
        )));
    }
    Ok((ns, param))
}

/// Best-effort type inference from a raw CLI string literal.
///
/// Precedence: `true`/`false` (case-insensitive) → bool; `i64`-parseable →
/// integer; `[a, b, c]` → list-of-string; everything else → string.
///
/// **Limitation:** the list parser splits on bare commas and only strips one
/// pair of outer `"`. Quoted commas inside list items (`["a,b", "c"]`) are
/// mis-tokenized into three items. For complex list literals, edit the
/// namespace's `aenv.toml` directly — `aenv set` is an ergonomic shortcut,
/// not a full TOML expression evaluator.
fn infer_value(literal: &str) -> ParameterValue {
    let trimmed = literal.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return ParameterValue::Boolean(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return ParameterValue::Boolean(false);
    }
    if let Ok(n) = trimmed.parse::<i64>() {
        return ParameterValue::Integer(n);
    }
    if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let xs: Vec<String> = inner
            .split(',')
            .map(|item| {
                let t = item.trim();
                t.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(t)
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();
        return ParameterValue::ListString(xs);
    }
    ParameterValue::String(trimmed.to_string())
}
