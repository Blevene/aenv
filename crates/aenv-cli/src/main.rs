//! `aenv` command-line entry point.

use aenv_core::fs::RealFilesystem;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

mod cmd;
mod paths;

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
    },
    /// List every namespace in the registry.
    List,
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
    },
    /// Adapter operations.
    Adapter {
        #[command(subcommand)]
        action: AdapterAction,
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
    List,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let fs = RealFilesystem;

    let result = (|| -> aenv_core::Result<()> {
        let layout = aenv_core::home::RegistryLayout::new(paths::resolve_aenv_home()?);
        aenv_core::adapters_builtin::ensure_written(&fs, &layout.adapters_dir())?;
        match cli.command {
            Command::Create { name } => cmd::create::run(&fs, &layout, &name),
            Command::List => cmd::list::run(&fs, &layout),
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
            Command::Status { project } => {
                let project_root = paths::resolve_project_root(&fs, project)?;
                cmd::status::run(&fs, &project_root)
            }
            Command::Adapter { action } => match action {
                AdapterAction::Add { path } => cmd::adapter::run_add(&fs, &layout, &path),
                AdapterAction::List => cmd::adapter::run_list(&fs, &layout),
            },
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
