//! `aenv` command-line entry point.

use aenv_cli::{cmd, paths};
use aenv_core::fs::RealFilesystem;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "aenv",
    version,
    about = "Virtual environments for AI coding harness configs",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new namespace in the registry.
    Create {
        /// Name of the namespace.
        name: String,
        /// Parent namespace(s) to extend. Repeatable: --extends base --extends shared.
        #[arg(long)]
        extends: Vec<String>,
        /// Seed an empty adapter block in the manifest. Repeatable: --adapter claude-code.
        /// Each name is validated against installed adapters (exit 11 if unknown).
        #[arg(long)]
        adapter: Vec<String>,
    },
    /// List every namespace in the registry.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Delete a namespace from the registry.
    Delete {
        /// Name of the namespace.
        name: String,
    },
    /// Pin the current project to a namespace by writing `.aenv`.
    Use {
        /// Name of the namespace.
        name: String,
        /// Project root override (defaults to ancestor walk from cwd).
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Activate the pinned namespace (or a named one) in a project.
    Activate {
        /// Namespace name (defaults to the .aenv pin).
        name: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Deactivate the active namespace in a project.
    Deactivate {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Restore the most recent backup set in a project.
    Restore {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Show the active namespace and managed files in a project.
    Status {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Adapter operations.
    Adapter {
        #[command(subcommand)]
        action: AdapterAction,
    },
    /// Show which namespace manages a given file path.
    Which {
        /// Project-relative path to query.
        path: PathBuf,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Print the effective value of a parameter plus provenance.
    ///
    /// SPEC is either `<namespace>.<param>` (explicit namespace) or
    /// `.<param>` (active project's pinned namespace).
    Get {
        /// Parameter spec: `<namespace>.<param>` or `.<param>`.
        spec: String,
        /// Project root override for the `.<param>` form (ancestor walk from cwd otherwise).
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Set a parameter on a namespace.
    Set {
        /// `<namespace>.<parameter>`
        spec: String,
        /// Value literal (type inferred: true/false → bool, digits → int,
        /// "[a, b]" → list-of-string, else string).
        value: String,
    },
    /// Evaluate policies for a namespace and report.
    ///
    /// Without an argument, uses the active project's pinned namespace (exit 20
    /// if not pinned). With a namespace name, evaluates that namespace directly.
    /// Exits 17 if any `enforce = true` policy is violated.
    Doctor {
        /// Namespace to evaluate (defaults to the active project's pinned namespace).
        namespace: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Detach a file (or whole project) from namespace management.
    ///
    /// With no argument: detach all managed files and remove .aenv-state/.
    /// With a project-relative path: detach only that file.
    /// With a namespace name: clone the namespace into a private fork (Task 15).
    Fork {
        /// File path or namespace name to fork (omit for whole-project detach).
        target: Option<PathBuf>,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Capture the current project's adapter-managed files into a new namespace.
    ///
    /// Walks each installed adapter's files = [...] patterns against the project
    /// tree and copies every matching file into a fresh namespace directory.
    /// The project pin is NOT updated. Refuses on duplicate name (exit 12) and
    /// on a project with no adapter-managed files.
    Snapshot {
        /// Name for the new namespace.
        name: String,
        /// Project root override (defaults to ancestor walk from cwd).
        #[arg(long)]
        project: Option<PathBuf>,
        /// Parent namespace(s) to extend. Repeatable: --extends base --extends shared.
        #[arg(long)]
        extends: Vec<String>,
    },
    /// Remove the .aenv pin from a project. If a namespace is currently
    /// active, runs the deactivate flow first.
    Unpin {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Skill operations.
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Diff against the active namespace (drift) or between two namespaces.
    Diff {
        /// First namespace name for structural diff (omit for drift).
        ns_a: Option<String>,
        /// Second namespace name for structural diff.
        ns_b: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AdapterAction {
    /// Install an adapter from a TOML file.
    Add {
        /// Source file.
        path: PathBuf,
    },
    /// List installed adapters.
    List {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SkillAction {
    /// Scaffold a new authored skill in a namespace.
    New {
        /// Skill name (becomes the directory name).
        name: String,
        /// Target namespace.
        #[arg(long)]
        ns: String,
        /// Adapter (defaults to the namespace's only adapter if exactly one).
        #[arg(long)]
        adapter: Option<String>,
    },
    /// Import a skill from a local path, git URL, or registry.
    Import {
        /// Source: /abs/path, ~/path, git+URL[#ref], or registry:<name>.
        source: String,
        #[arg(long)]
        ns: String,
        #[arg(long)]
        adapter: Option<String>,
        #[arg(long)]
        pin: Option<String>,
        /// Optional sub-path inside the source. Pick one skill out of a
        /// monorepo whose layout is `<path>/<name>/SKILL.md` (e.g.
        /// `--path scientific-skills/scanpy`).
        #[arg(long)]
        path: Option<String>,
    },
    /// List every skill in every namespace (or one if --ns).
    List {
        #[arg(long)]
        ns: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let fs = RealFilesystem;

    let result = (|| -> aenv_core::Result<()> {
        let layout = aenv_core::home::RegistryLayout::new(paths::resolve_aenv_home()?);
        aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir())?;
        aenv_core::namespaces_builtin::ensure_written(&fs, &layout)?;
        match cli.command {
            Command::Create {
                name,
                extends,
                adapter,
            } => {
                let adapters_reg = aenv_core::adapter::AdapterRegistry::load_from_dir(
                    &fs,
                    &layout.adapters_dir(),
                )?;
                cmd::create::run(&fs, &layout, &adapters_reg, &name, &extends, &adapter)
            }
            Command::List { json } => cmd::list::run(&fs, &layout, json),
            Command::Delete { name } => cmd::delete::run(&fs, &layout, &name),
            Command::Use { name, project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::use_::run(&fs, &layout, &project_root, &name)
            }
            Command::Activate { name, project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::activate::run(&fs, &layout, &project_root, name.as_deref())
            }
            Command::Deactivate { project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::deactivate::run(&fs, &project_root)
            }
            Command::Restore { project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::restore::run(&fs, &project_root)
            }
            Command::Status { project, json } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                let aenv_home = paths::resolve_aenv_home()?;
                cmd::status::run(&fs, &project_root, &aenv_home, json)
            }
            Command::Adapter { action } => match action {
                AdapterAction::Add { path } => cmd::adapter::run_add(&fs, &layout, &path),
                AdapterAction::List { json } => cmd::adapter::run_list(&fs, &layout, json),
            },
            Command::Get {
                spec,
                project,
                json,
            } => {
                let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                    &fs,
                    &layout.adapters_dir(),
                )?;
                // For the explicit `ns.param` form project root is irrelevant;
                // for `.<param>` it's needed — resolve it only when --project
                // was supplied (otherwise cmd::get::run walks cwd itself).
                let project_root_hint = project
                    .map(|p| -> aenv_core::Result<std::path::PathBuf> {
                        paths::resolve_project_root(&fs, Some(p))
                    })
                    .transpose()?;
                cmd::get::run(
                    &fs,
                    &layout,
                    &adapters,
                    project_root_hint.as_deref(),
                    &spec,
                    json,
                )
            }
            Command::Set { spec, value } => cmd::set::run(&fs, &layout, &spec, &value),
            Command::Doctor {
                namespace,
                project,
                json,
            } => {
                let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                    &fs,
                    &layout.adapters_dir(),
                )?;
                // Resolve project root only when needed (namespace arg absent).
                // When an explicit namespace is given, project root is irrelevant;
                // avoid failing with ProjectNotPinned for the explicit-ns form.
                let project_root = if namespace.is_some() && project.is_none() {
                    std::env::current_dir().map_err(aenv_core::AenvError::Io)?
                } else {
                    paths::resolve_project_root(&fs, project)?
                };
                cmd::doctor::run(
                    &fs,
                    &layout,
                    &adapters,
                    &project_root,
                    namespace.as_deref(),
                    json,
                )
            }
            Command::Which {
                path,
                project,
                json,
            } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                let aenv_home = paths::resolve_aenv_home()?;
                cmd::which::run(project_root, path, &aenv_home, json)
            }
            Command::Unpin { project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::unpin::run(&fs, &project_root)
            }
            Command::Skill { action } => match action {
                SkillAction::New { name, ns, adapter } => {
                    let adapters_reg = aenv_core::adapter::AdapterRegistry::load_from_dir(
                        &fs,
                        &layout.adapters_dir(),
                    )?;
                    cmd::skill::new::run(
                        &fs,
                        &layout,
                        &adapters_reg,
                        &ns,
                        &name,
                        adapter.as_deref(),
                    )
                }
                SkillAction::Import {
                    source,
                    ns,
                    adapter,
                    pin,
                    path,
                } => {
                    let adapters_reg = aenv_core::adapter::AdapterRegistry::load_from_dir(
                        &fs,
                        &layout.adapters_dir(),
                    )?;
                    cmd::skill::import::run(
                        &fs,
                        &layout,
                        &adapters_reg,
                        &ns,
                        &source,
                        adapter.as_deref(),
                        pin.as_deref(),
                        path.as_deref(),
                    )
                }
                SkillAction::List { ns, json } => {
                    cmd::skill::list::run(&fs, &layout, ns.as_deref(), json)
                }
            },
            Command::Diff {
                ns_a,
                ns_b,
                project,
                json,
            } => match (ns_a, ns_b) {
                (None, None) => {
                    let project_root = paths::resolve_project_root(&fs, project)?;
                    let aenv_home = paths::resolve_aenv_home()?;
                    cmd::diff::run_drift(&fs, &project_root, &aenv_home, json)
                }
                (Some(a), Some(b)) => cmd::diff::run_structural(&fs, &layout, &a, &b, json),
                _ => Err(aenv_core::AenvError::ManifestInvalid(
                    "aenv diff needs either zero or two namespace arguments".into(),
                )),
            },
            Command::Snapshot {
                name,
                project,
                extends,
            } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                    &fs,
                    &layout.adapters_dir(),
                )?;
                cmd::snapshot::run(&fs, &layout, &adapters, &project_root, &name, &extends)
            }
            Command::Fork { target, project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                match target {
                    None => cmd::fork::run_project_detach(project_root),
                    Some(t) => {
                        let rel = t.clone();
                        let project_path = project_root.join(&rel);
                        let state_path = project_root.join(".aenv-state/state.json");
                        let is_managed = std::fs::read(&state_path)
                            .ok()
                            .and_then(|b| {
                                String::from_utf8(b).ok().and_then(|s| {
                                    aenv_core::state::ActivationState::from_json(&s).ok()
                                })
                            })
                            .is_some_and(|state| state.managed_files.iter().any(|m| m.path == rel));
                        if is_managed || project_path.exists() {
                            cmd::fork::run_file(project_root, rel)
                        } else {
                            let aenv_home = paths::resolve_aenv_home()?;
                            cmd::fork::run_name(
                                aenv_home,
                                project_root,
                                t.to_string_lossy().into_owned(),
                            )
                        }
                    }
                }
            }
        }
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}
