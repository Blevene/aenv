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
        /// Create a user-scope (global) namespace instead of a project one.
        /// Equivalent to `aenv global new`: seeds the adapter's instructions
        /// file under the namespace's `user/` subtree and pre-wires
        /// `user_files`. `--extends` is not supported with `--global` yet.
        #[arg(long)]
        global: bool,
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
    /// Pin the current project to a namespace by writing a `.aenv` file
    /// at the project root. Does NOT materialize any files — follow with
    /// `aenv activate` to symlink the namespace's content into the project.
    Use {
        /// Name of the namespace.
        name: String,
        /// Project root override (defaults to ancestor walk from cwd).
        #[arg(long)]
        project: Option<PathBuf>,
        /// Also activate the namespace fully on both surfaces. Sugar for
        /// `aenv use <ns> && aenv activate && aenv global activate <ns>`.
        #[arg(long)]
        global: bool,
        /// Approve any `[lifecycle].on_activate` script without prompting.
        /// Records the approval as if the user had answered "yes" — future
        /// activations with an unchanged script proceed silently. Only
        /// meaningful in combination with `--global`.
        #[arg(long)]
        yes: bool,
    },
    /// Materialize the active namespace's content into the project as
    /// symlinks (or merged files where strategy demands). Reads the
    /// namespace name from the `.aenv` pin unless one is passed
    /// explicitly. Backs up any displaced originals to
    /// `.aenv-state/backup/<timestamp>/`.
    Activate {
        /// Namespace name (defaults to the .aenv pin for project scope;
        /// required with `--global`).
        name: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        /// Activate the namespace's user-scope surface into `$HOME`
        /// (`~/.claude/`, `~/.codex/`, …) instead of the project. Routes to
        /// the same core as `aenv global use <name>`, including baseline
        /// capture, pre-flight, and lifecycle approval.
        #[arg(long)]
        global: bool,
        /// Approve lifecycle scripts and proceed past pre-flight findings
        /// without prompting. Only meaningful with `--global`.
        #[arg(long)]
        yes: bool,
        /// Skip the first-activation baseline capture. Only meaningful with
        /// `--global`.
        #[arg(long)]
        no_baseline: bool,
    },
    /// Reverse `aenv activate`: remove every file aenv materialized,
    /// restore any backed-up originals byte-for-byte, and clear the active
    /// state. Leaves the `.aenv` pin file in place (use `aenv unpin` to remove
    /// that too) and retains the `.aenv-state/backup/<ts>/` scaffolding unless
    /// `--prune` is passed.
    Deactivate {
        #[arg(long)]
        project: Option<PathBuf>,
        /// Also remove every timestamped backup directory under
        /// `.aenv-state/backup/`. Older runs' backups accumulate
        /// otherwise. (The global-scope analog is `aenv global doctor --fix`.)
        /// Project scope only.
        #[arg(long)]
        prune: bool,
        /// Deactivate the user-scope (global) activation in `$HOME` instead of
        /// the project. Routes to the same core as `aenv global deactivate`.
        #[arg(long)]
        global: bool,
        /// Skip `on_deactivate` lifecycle hooks (for when a hook itself is
        /// broken). File restoration proceeds either way. Only meaningful with
        /// `--global`.
        #[arg(long)]
        force: bool,
    },
    /// Recovery path when `aenv deactivate` didn't run cleanly. Copies
    /// the most recent `.aenv-state/backup/<timestamp>/` set back into
    /// the project. Uses copy semantics (not move) so the backup is
    /// re-runnable. Errors if no backup exists.
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
    /// Cache operations (currently: prune unreferenced skill clones).
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
    /// Print a shell hook script for sourcing in your rc file. The hook
    /// auto-activates the right namespace as you `cd` between projects.
    ///
    /// Usage: `eval "$(aenv init-shell <bash|zsh|fish>)"` (or `| source` for fish).
    InitShell {
        /// Shell to emit a hook for.
        shell: String,
    },
    /// Fast-path the shell hook calls on every chpwd. Walks ancestors for
    /// a `.aenv` pin and transitions to the right namespace. Prints the
    /// new active project root (or empty line if none) to stdout.
    ///
    /// Not intended for direct user invocation.
    ActivateIfNeeded {
        /// Project root the previous invocation activated; pass an empty
        /// string when nothing was active (typical first shell-hook call).
        #[arg(default_value = "")]
        last_active: String,
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
    /// User-global activation surface (`~/.claude/`, `~/.codex/`, …).
    /// Mirrors the project-local verbs but operates on `$HOME` instead of
    /// the project root. One activation lives per user; activating a new
    /// namespace deactivates the prior one in a single transaction.
    Global {
        #[command(subcommand)]
        action: GlobalAction,
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
        /// Scope: `project` (default) or `user`. `user` scaffolds the skill
        /// under the adapter's user_skills_dir so it materializes into
        /// `~/.claude/skills/<name>/` when the namespace is activated globally.
        #[arg(long, default_value = "project")]
        scope: String,
    },
    /// Import a skill from a local path, git URL, or registry.
    Import {
        /// Source: `/abs/path`, `~/path`, `git+URL[#ref]`, or `registry:<name>`.
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
        /// Scope: `project` (default) or `user`. `user` makes the skill
        /// materialize into `~/.claude/skills/<name>/` when the namespace is
        /// activated globally (`aenv global use <ns>`).
        #[arg(long, default_value = "project")]
        scope: String,
    },
    /// List every skill in every namespace (or one if --ns).
    List {
        #[arg(long)]
        ns: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Remove a skill from a namespace.
    ///
    /// Deletes the `[[skills]]` entry from the manifest. For authored
    /// skills, also removes the on-disk skill directory. For imported
    /// skills, the `~/.aenv/cache/skills/` clone is left in place — run
    /// `aenv cache prune` to reclaim space.
    Remove {
        /// Skill name to remove.
        name: String,
        /// Target namespace.
        #[arg(long)]
        ns: String,
    },
}

#[derive(Debug, Subcommand)]
enum CacheAction {
    /// Walk every namespace's `[[skills]]` entries, collect the
    /// (source-hash, ref) cache directories in use, delete the rest.
    Prune,
}

#[derive(Debug, Subcommand)]
enum GlobalAction {
    /// Switch your active global profile. `<target>` may be a git URL or
    /// local path (imported on the spot if not already present), an existing
    /// namespace name, or `-` to toggle back to the previous profile. This is
    /// the one-command front door: it folds import + activate + swap into a
    /// single step. On the first-ever activation it captures a `baseline`
    /// (opt out with --no-baseline).
    Use {
        /// git URL, local path, existing namespace name, or `-` (previous).
        target: String,
        /// Name to give an imported source (defaults to the derived name).
        /// Ignored when the target is an existing namespace or `-`.
        #[arg(long = "as")]
        as_name: Option<String>,
        /// Pin a git source to a tag, commit, or branch. git URLs only.
        #[arg(long)]
        pin: Option<String>,
        /// Non-interactive: approve lifecycle scripts and proceed past
        /// pre-flight findings without prompting. See `activate --yes`.
        #[arg(long)]
        yes: bool,
        /// Skip the first-activation baseline capture.
        #[arg(long)]
        no_baseline: bool,
    },
    /// DEPRECATED: use `aenv global use <name>` instead. Activates a
    /// namespace's user-scope files into `$HOME`, replacing any existing
    /// activation in one transaction. `use` is a superset — it also imports
    /// git/path sources on the spot and records a swap-back point. This alias
    /// still works but prints a deprecation notice.
    Activate {
        /// Namespace name to activate globally.
        name: String,
        /// Non-interactive: approve any `[lifecycle].on_activate` script and
        /// proceed past pre-flight findings without prompting. The approval
        /// is recorded as if you had answered "yes" — future activations of
        /// the same namespace with an unchanged script proceed silently.
        /// The pre-flight scan still runs and reports its findings; this
        /// flag only suppresses the prompt. Use with caution: lifecycle
        /// scripts run with your user privileges.
        #[arg(long)]
        yes: bool,
        /// Skip the first-activation baseline capture. By default, the very
        /// first global activation snapshots your current `$HOME` surface
        /// into a `baseline` namespace so you always have a named return
        /// point. Pass this to opt out.
        #[arg(long)]
        no_baseline: bool,
    },
    /// Reverse `aenv global activate`: restore stashed originals, delete the
    /// global state file. Exit 0 with a note if no activation is live.
    Deactivate {
        /// Skip the namespace's `on_deactivate` lifecycle hook. Use when the
        /// hook itself is broken (e.g. it depends on a runtime that is
        /// missing or corrupted). File restoration proceeds either way.
        #[arg(long)]
        force: bool,
    },
    /// Show the active global namespace and managed files.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Show which global namespace manages a given user-scope path.
    Which {
        /// Path to query (absolute or relative to `$HOME`).
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// List only namespaces that declare user-scope files (`user_files` or
    /// `scope = "user"` skills). To see every namespace regardless of scope,
    /// use `aenv list`.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Evaluate policies against a namespace's user-scope candidates, and
    /// audit global state for orphan stash directories.
    Doctor {
        /// Namespace to evaluate (defaults to the active global namespace).
        namespace: Option<String>,
        #[arg(long)]
        json: bool,
        /// Delete any orphan stash directories found during the audit
        /// (subdirs of `<aenv_home>/global-stash/` not referenced by the
        /// active state), then report clean. Without this, orphans are an
        /// error (exit 19) when auditing global state as a whole.
        #[arg(long)]
        fix: bool,
    },
    /// Scaffold a new, empty user-scope namespace ready to edit. Seeds the
    /// adapter's instructions file (e.g. `~/.claude/CLAUDE.md`) under the
    /// namespace's `user/` subtree and pre-wires the manifest's `user_files`.
    /// Author your own global profile from scratch, then turn it on with
    /// `aenv global use <name>`.
    New {
        /// Name of the new namespace. Must be a valid `NamespaceId` and not
        /// already exist.
        name: String,
        /// Adapter to scaffold for.
        #[arg(long, default_value = "claude-code")]
        adapter: String,
    },
    /// Snapshot the current `$HOME` user-scope surface (`~/.claude/`,
    /// `~/.codex/`, etc.) into a new namespace. The set of captured paths
    /// is determined by every installed adapter's `user_files` plus any
    /// `--include` extras.
    ///
    /// The resulting namespace is byte-identical when re-activated: the
    /// strategy on the next `aenv global activate <name>` is `Identical`
    /// (no backup needed).
    Snapshot {
        /// Name of the new namespace. Must be a valid `NamespaceId` and not
        /// already exist.
        name: String,
        /// Extra paths (relative to `$HOME`) to include beyond adapter
        /// defaults. Repeatable: `--include .claude/runtime --include .claude/bin`.
        #[arg(long)]
        include: Vec<String>,
    },
    /// Import a source directory or git URL as a new namespace. When the
    /// source root contains `aenv-namespace.toml`, its declared `[layout]`
    /// and `[lifecycle]` are authoritative; otherwise a built-in heuristic
    /// probes well-known config paths (CLAUDE.md, agents/, hooks/, skills/,
    /// settings.json, …). The heuristic imports config only — it never wires
    /// a repo's install.sh as a lifecycle hook; declare hooks explicitly in
    /// `aenv-namespace.toml` if you want them.
    ///
    /// The resulting namespace can be activated with `aenv global use
    /// <name>`. See `pm_docs/aenv-namespace-toml-spec.md` for the convention
    /// file format.
    Import {
        /// Source: a local filesystem path, or a git URL
        /// (https://, http://, git://, git@, file://, any URL ending in
        /// `.git`, or any of these with a `git+` prefix as `aenv skill import`
        /// uses).
        source: String,
        /// Namespace name. Defaults to the last path component of `source`
        /// for local paths, or the repo name (with trailing `.git` stripped)
        /// for git URLs.
        #[arg(default_value = "")]
        name: String,
        /// Pin a git source to a specific tag, commit, or branch. Only
        /// applies to git URL sources.
        #[arg(long)]
        pin: Option<String>,
    },
    /// Diff user-scope content against the active global activation or
    /// between two namespaces' user-scope subsets.
    Diff {
        /// First namespace name for structural diff (omit for drift).
        ns_a: Option<String>,
        /// Second namespace name for structural diff.
        ns_b: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

/// Parse a `--scope` flag value into a `Scope`. Accepts only `project` or
/// `user`; anything else is a `ManifestInvalid` (exit 12).
fn parse_scope(s: &str) -> Result<aenv_core::scope::Scope, aenv_core::AenvError> {
    match s {
        "project" => Ok(aenv_core::scope::Scope::Project),
        "user" => Ok(aenv_core::scope::Scope::User),
        other => Err(aenv_core::AenvError::ManifestInvalid(format!(
            "invalid --scope '{other}'; expected 'project' or 'user'"
        ))),
    }
}

/// Resolve the user-scope target root (`$HOME`) for `--global` operations.
/// User-scope activation materializes into `~/.claude/`, `~/.codex/`, … so it
/// needs `HOME`; absence is a hard error rather than a silent default.
fn fake_home() -> Result<std::path::PathBuf, aenv_core::AenvError> {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .map_err(|_| {
            aenv_core::AenvError::ManifestInvalid("HOME not set; --global requires HOME".into())
        })
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
                global,
            } => {
                if global {
                    // `aenv create --global` scaffolds a user-scope namespace,
                    // the same as `aenv global new`. The global scaffolder is
                    // single-adapter and has no `extends` path yet.
                    if !extends.is_empty() {
                        return Err(aenv_core::AenvError::ManifestInvalid(
                            "--extends is not supported with --global; create the \
                             namespace then edit its manifest"
                                .into(),
                        ));
                    }
                    if adapter.len() > 1 {
                        return Err(aenv_core::AenvError::ManifestInvalid(
                            "--global scaffolds a single adapter; pass at most one --adapter"
                                .into(),
                        ));
                    }
                    let adapter_name = adapter.first().map(String::as_str).unwrap_or("claude-code");
                    return cmd::global::new::run(&fs, &layout, &name, adapter_name);
                }
                let adapters_reg = aenv_core::adapter::AdapterRegistry::load_from_dir(
                    &fs,
                    &layout.adapters_dir(),
                )?;
                cmd::create::run(&fs, &layout, &adapters_reg, &name, &extends, &adapter)
            }
            Command::List { json } => cmd::list::run(&fs, &layout, json),
            Command::Delete { name } => cmd::delete::run(&fs, &layout, &name),
            Command::Use {
                name,
                project,
                global,
                yes,
            } => {
                let project_root = paths::resolve_project_root_for_pin(&fs, project)?;
                cmd::use_::run(&fs, &layout, &project_root, &name)?;
                if global {
                    // --global expands to: pin (above), then activate the
                    // project, then activate globally. Order matches
                    // reversibility — pin is cheapest to undo, global is
                    // the most observable side effect.
                    cmd::activate::run(&fs, &layout, &project_root, Some(name.as_str()))?;
                    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                        &fs,
                        &layout.adapters_dir(),
                    )?;
                    // `aenv use --global <ns>` forwards the user's `--yes`:
                    // with it, the whole sequence (pin, activate project,
                    // activate global) runs non-interactively; without it,
                    // the pre-flight and lifecycle-approval gates prompt as
                    // usual. We never auto-approve a lifecycle script the
                    // user didn't consent to. Baseline capture stays enabled
                    // (the safer default).
                    cmd::global::activate::run(
                        &fs,
                        &layout,
                        &adapters,
                        &fake_home()?,
                        &name,
                        yes,
                        false,
                    )?;
                }
                Ok(())
            }
            Command::Activate {
                name,
                project,
                global,
                yes,
                no_baseline,
            } => {
                if global {
                    if project.is_some() {
                        return Err(aenv_core::AenvError::ManifestInvalid(
                            "--project (a project path) cannot be combined with --global".into(),
                        ));
                    }
                    let name = name.ok_or_else(|| {
                        aenv_core::AenvError::ManifestInvalid(
                            "--global activation needs a namespace name: aenv activate <ns> --global"
                                .into(),
                        )
                    })?;
                    let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                        &fs,
                        &layout.adapters_dir(),
                    )?;
                    return cmd::global::activate::run(
                        &fs,
                        &layout,
                        &adapters,
                        &fake_home()?,
                        &name,
                        yes,
                        no_baseline,
                    );
                }
                if yes || no_baseline {
                    return Err(aenv_core::AenvError::ManifestInvalid(
                        "--yes / --no-baseline only apply with --global".into(),
                    ));
                }
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::activate::run(&fs, &layout, &project_root, name.as_deref())
            }
            Command::Deactivate {
                project,
                prune,
                global,
                force,
            } => {
                if global {
                    if project.is_some() {
                        return Err(aenv_core::AenvError::ManifestInvalid(
                            "--project (a project path) cannot be combined with --global".into(),
                        ));
                    }
                    if prune {
                        return Err(aenv_core::AenvError::ManifestInvalid(
                            "--prune is project scope only; clear global stashes with \
                             `aenv global doctor --fix`"
                                .into(),
                        ));
                    }
                    return cmd::global::deactivate::run(&fs, &layout, &fake_home()?, force);
                }
                if force {
                    return Err(aenv_core::AenvError::ManifestInvalid(
                        "--force only applies with --global".into(),
                    ));
                }
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::deactivate::run(&fs, &project_root, prune)
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
                cmd::which::run(&fs, project_root, path, &aenv_home, json)
            }
            Command::Unpin { project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::unpin::run(&fs, &project_root)
            }
            Command::Skill { action } => match action {
                SkillAction::New {
                    name,
                    ns,
                    adapter,
                    scope,
                } => {
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
                        parse_scope(&scope)?,
                    )
                }
                SkillAction::Import {
                    source,
                    ns,
                    adapter,
                    pin,
                    path,
                    scope,
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
                        parse_scope(&scope)?,
                    )
                }
                SkillAction::List { ns, json } => {
                    cmd::skill::list::run(&fs, &layout, ns.as_deref(), json)
                }
                SkillAction::Remove { name, ns } => {
                    cmd::skill::remove::run(&fs, &layout, &ns, &name)
                }
            },
            Command::Cache { action } => match action {
                CacheAction::Prune => cmd::cache::run_prune(&fs, &layout),
            },
            Command::InitShell { shell } => cmd::init_shell::run(&shell),
            Command::ActivateIfNeeded { last_active } => {
                let last_path;
                let last = if last_active.is_empty() {
                    None
                } else {
                    last_path = std::path::PathBuf::from(&last_active);
                    Some(last_path.as_path())
                };
                cmd::activate_if_needed::run(&fs, &layout, last)
            }
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
                // snapshot captures an as-yet-unmanaged project, so it must not
                // require a `.aenv` pin — fall back to cwd when none exists.
                let project_root = paths::resolve_project_root_or_cwd(&fs, project)?;
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
            Command::Global { action } => {
                let fake_home = std::env::var("HOME")
                    .map(std::path::PathBuf::from)
                    .map_err(|_| {
                        aenv_core::AenvError::ManifestInvalid(
                            "HOME not set; aenv global requires HOME".into(),
                        )
                    })?;
                match action {
                    GlobalAction::Use {
                        target,
                        as_name,
                        pin,
                        yes,
                        no_baseline,
                    } => {
                        let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                            &fs,
                            &layout.adapters_dir(),
                        )?;
                        cmd::global::use_::run(
                            &fs,
                            &layout,
                            &adapters,
                            &fake_home,
                            &target,
                            as_name.as_deref(),
                            pin.as_deref(),
                            yes,
                            no_baseline,
                        )
                    }
                    GlobalAction::Activate {
                        name,
                        yes,
                        no_baseline,
                    } => {
                        eprintln!(
                            "warning: `aenv global activate` is deprecated; use \
                             `aenv global use {name}` instead."
                        );
                        let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                            &fs,
                            &layout.adapters_dir(),
                        )?;
                        cmd::global::activate::run(
                            &fs,
                            &layout,
                            &adapters,
                            &fake_home,
                            &name,
                            yes,
                            no_baseline,
                        )
                    }
                    GlobalAction::Deactivate { force } => {
                        cmd::global::deactivate::run(&fs, &layout, &fake_home, force)
                    }
                    GlobalAction::Status { json } => {
                        cmd::global::status::run(&fs, &layout, &fake_home, json)
                    }
                    GlobalAction::Which { path, json } => {
                        let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                            &fs,
                            &layout.adapters_dir(),
                        )?;
                        cmd::global::which::run(&fs, &layout, &adapters, &fake_home, &path, json)
                    }
                    GlobalAction::List { json } => cmd::global::list::run(&fs, &layout, json),
                    GlobalAction::Doctor {
                        namespace,
                        json,
                        fix,
                    } => {
                        let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                            &fs,
                            &layout.adapters_dir(),
                        )?;
                        cmd::global::doctor::run(
                            &fs,
                            &layout,
                            &adapters,
                            &fake_home,
                            namespace.as_deref(),
                            json,
                            fix,
                        )
                    }
                    GlobalAction::New { name, adapter } => {
                        cmd::global::new::run(&fs, &layout, &name, &adapter)
                    }
                    GlobalAction::Snapshot { name, include } => {
                        let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                            &fs,
                            &layout.adapters_dir(),
                        )?;
                        cmd::global::snapshot::run(
                            &fs, &layout, &adapters, &fake_home, &name, &include,
                        )
                    }
                    GlobalAction::Import { source, name, pin } => {
                        let adapters = aenv_core::adapter::AdapterRegistry::load_from_dir(
                            &fs,
                            &layout.adapters_dir(),
                        )?;
                        cmd::global::import::run(
                            &fs,
                            &layout,
                            &adapters,
                            &source,
                            &name,
                            pin.as_deref(),
                        )
                    }
                    GlobalAction::Diff { ns_a, ns_b, json } => cmd::global::diff::run(
                        &fs,
                        &layout,
                        &fake_home,
                        ns_a.as_deref(),
                        ns_b.as_deref(),
                        json,
                    ),
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
