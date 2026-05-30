//! `aenv skill import <source> --ns <ns> [--adapter <a>] [--pin <ref>]`

use aenv_core::adapter::AdapterRegistry;
use aenv_core::error::{AenvError, Result};
use aenv_core::fs::Filesystem;
use aenv_core::home::RegistryLayout;
use aenv_core::manifest::AenvManifest;
use aenv_core::skills::{apply_required_rule, SkillDecl, SkillMode};

// The 8th argument is `path_arg` (Phase 4 sub-path support). Wrapping these
// in a struct would obscure the call site without simplifying the logic.
#[allow(clippy::too_many_arguments)]
pub fn run<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    adapters: &AdapterRegistry,
    namespace: &str,
    source: &str,
    adapter_arg: Option<&str>,
    pin: Option<&str>,
    path_arg: Option<&str>,
    scope: aenv_core::scope::Scope,
) -> Result<()> {
    let manifest_path = layout.manifest_path(namespace);
    if !fs.exists(&manifest_path)? {
        return Err(AenvError::NamespaceNotFound(namespace.to_string()));
    }
    let bytes = fs.read(&manifest_path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| AenvError::ManifestInvalid(format!("manifest not utf-8: {e}")))?;
    let mut manifest = AenvManifest::from_toml(text)?;

    let adapter_name = match adapter_arg {
        Some(a) => a.to_string(),
        None => {
            if manifest.adapters.len() != 1 {
                return Err(AenvError::ManifestInvalid(format!(
                    "namespace '{namespace}' declares {} adapters; use --adapter to disambiguate",
                    manifest.adapters.len()
                )));
            }
            manifest.adapters.keys().next().unwrap().clone()
        }
    };

    // Derive a skill name. With --path, the path's basename takes precedence
    // (`scientific-skills/scanpy` → `scanpy`) because the source URL alone
    // can't pick which skill out of a monorepo we want. Without --path, fall
    // back to the source-derived name (fragment, repo basename, etc.).
    let skill_name = match path_arg {
        Some(p) => skill_name_from_path(p),
        None => None,
    }
    .or_else(|| derive_skill_name(source))
    .ok_or_else(|| {
        AenvError::ManifestInvalid(format!(
            "could not derive a skill name from source '{source}'; \
             pick a different source, pass --path, or edit the manifest manually"
        ))
    })?;

    if manifest.skills.iter().any(|s| s.name == skill_name) {
        return Err(AenvError::ManifestInvalid(format!(
            "namespace '{namespace}' already declares a skill '{skill_name}'"
        )));
    }

    let mut decl = SkillDecl {
        name: skill_name.clone(),
        mode: SkillMode::Imported,
        adapter: Some(adapter_name),
        source: Some(source.to_string()),
        ref_: pin.map(String::from),
        path: path_arg.map(String::from),
        required: false,
        scope,
    };

    if let Some(pin_ref) = pin {
        eprintln!("Resolving {source} @ {pin_ref}...");
        if let Some(resolved_sha) = resolve_pin(fs, layout, &decl)? {
            decl.ref_ = Some(resolved_sha);
        }
    }

    let _ = adapters; // declarations don't need adapter lookup yet
    let pinned_ref = decl.ref_.clone();
    let pinned_path = decl.path.clone();
    manifest.skills.push(decl);
    fs.write(&manifest_path, manifest.to_toml().as_bytes())?;
    println!("Imported skill '{skill_name}' into namespace '{namespace}':");
    println!("  - source: {source}");
    println!("  - scope: {}", scope.as_str());
    if let Some(p) = pinned_path {
        println!("  - path: {p}");
    }
    if let Some(r) = pinned_ref {
        println!("  - pinned ref: {r}");
    } else {
        println!("  - no pin (resolves on each activation)");
    }
    println!("  - registered in {}", manifest_path.display());
    Ok(())
}

/// `scientific-skills/scanpy` → `scanpy`; bare `scanpy` → `scanpy`.
/// Returns `None` for empty paths or those ending in a separator.
fn skill_name_from_path(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .map(std::string::ToString::to_string)
}

/// Resolve an imported skill once to verify reachability and capture the
/// concrete commit SHA when the user pinned by branch or tag. Returns
/// `Ok(Some(sha))` when a git source returned a resolved ref, or
/// `Ok(None)` for local / registry sources that have no SHA. Propagates
/// errors so the import fails before writing the manifest.
///
/// Internally promotes the decl to `required = true` so resolution failure
/// surfaces as `Err`; this is the only call site that does this transient
/// promotion, and it never escapes the function.
fn resolve_pin<F: Filesystem>(
    fs: &F,
    layout: &RegistryLayout,
    decl: &SkillDecl,
) -> Result<Option<String>> {
    let mut required_decl = decl.clone();
    required_decl.required = true;
    let resolution = apply_required_rule(fs, layout, &required_decl)?
        .expect("required=true should propagate errors as Err");
    Ok(resolution.resolved_ref)
}

/// Derive a default skill name from the source string.
///
/// Convention (per functional spec §5.10):
/// * `git+<url>#<fragment>` → the fragment. The spec example
///   `git+...aenv-skills.git#match-conventions` imports the skill named
///   `match-conventions` from the repo. Pinning uses `--pin <ref>` for the
///   git ref, not the fragment. If your fragment happens to look like a
///   branch name (e.g. `#main`), you'll get a skill literally named "main" —
///   edit the manifest by hand or omit the fragment to use the repo name.
/// * `git+<url>` (no fragment) → last path component, with `.git` stripped.
/// * `registry:<name>` → `<name>`.
/// * Local path → last path component.
fn derive_skill_name(source: &str) -> Option<String> {
    if let Some(rest) = source.strip_prefix("git+") {
        if let Some((_, after_hash)) = rest.split_once('#') {
            return Some(after_hash.to_string());
        }
        let url_tail = rest.rsplit('/').next()?;
        return Some(url_tail.trim_end_matches(".git").to_string());
    }
    if let Some(name) = source.strip_prefix("registry:") {
        return Some(name.to_string());
    }
    // Local path: use last component.
    std::path::Path::new(source)
        .file_name()
        .and_then(|s| s.to_str())
        .map(std::string::ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::derive_skill_name;

    #[test]
    fn fragment_becomes_skill_name() {
        // Spec §5.10 convention: the fragment names the skill within the repo.
        assert_eq!(
            derive_skill_name("git+https://github.com/acme/aenv-skills.git#match-conventions"),
            Some("match-conventions".into())
        );
    }

    #[test]
    fn git_no_fragment_uses_repo_name_minus_dot_git() {
        assert_eq!(
            derive_skill_name("git+https://github.com/user/my-skill.git"),
            Some("my-skill".into())
        );
        assert_eq!(
            derive_skill_name("git+https://github.com/user/my-skill"),
            Some("my-skill".into())
        );
    }

    #[test]
    fn registry_uses_bare_name() {
        assert_eq!(
            derive_skill_name("registry:cite-evidence"),
            Some("cite-evidence".into())
        );
    }

    #[test]
    fn local_path_uses_last_component() {
        assert_eq!(
            derive_skill_name("/home/user/team-skills/check-before-submit"),
            Some("check-before-submit".into())
        );
        assert_eq!(derive_skill_name("~/team-skills/foo"), Some("foo".into()));
    }
}
