//! `aenv` command-line entry point.
//!
//! The binary is intentionally thin: parse arguments via clap, dispatch into
//! `aenv-core`, map the result to an exit code. No business logic lives here.

use clap::Parser;

mod paths;

/// Top-level CLI definition.
#[derive(Debug, Parser)]
#[command(
    name = "aenv",
    version,
    about = "Virtual environments for AI coding harness configs",
    long_about = None,
)]
struct Cli {
    // Subcommands land in later phases. For now, `--version` is the only
    // supported invocation; clap derives it from `version` above.
}

fn main() {
    let _cli = Cli::parse();
    // Phase 1 adds subcommand dispatch here. For now, if we reach this point
    // with no subcommand and clap has already handled --version, we exit 0.
}
