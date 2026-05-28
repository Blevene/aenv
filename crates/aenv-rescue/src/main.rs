//! `aenv-rescue` — emergency deactivate when the active namespace's hooks
//! have locked the user out of their shell.
//!
//! Reads `$AENV_HOME/global-state.json` (default `$HOME/.aenv/global-state.json`),
//! removes every materialized file, restores every backed-up original,
//! and tears down the state and lock files. **Never** spawns subprocesses;
//! never runs `on_deactivate`; never invokes Claude Code's hook chain.
//!
//! Run from any non-Claude shell when `aenv global deactivate` itself
//! fails because the active namespace's pre-tool-use hook blocks Bash.
//!
//! Exit codes:
//!   0 — success (or no active activation; idempotent).
//!   1 — state file present but unreadable / malformed.
//!   2 — unknown argument on the command line.

use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "\
aenv-rescue — emergency deactivate for aenv global namespaces.

USAGE:
    aenv-rescue            Restore the user's $HOME from the active global
                           activation by reading $AENV_HOME/global-state.json
                           directly. Never spawns subprocesses; never runs the
                           namespace's on_deactivate hook; never touches the
                           Claude Code hook chain.

    aenv-rescue --help     Print this message.
    aenv-rescue --version  Print version and exit.

ENVIRONMENT:
    AENV_HOME              Override aenv home (default: $HOME/.aenv).
    HOME                   Required when AENV_HOME is unset.

EXIT CODES:
    0  Success (or no active activation; idempotent).
    1  State file present but unreadable or malformed.
    2  Unknown command-line argument.

See `pm_docs/walkthrough-global-namespaces.md` for the full recovery flow.
";

fn aenv_home() -> PathBuf {
    if let Ok(explicit) = std::env::var("AENV_HOME") {
        return PathBuf::from(explicit);
    }
    let home = std::env::var("HOME").expect("HOME must be set; rescue cannot proceed without it");
    PathBuf::from(home).join(".aenv")
}

fn main() -> ExitCode {
    // Manual argv handling — no clap dependency. The binary is intentionally
    // tiny so it builds and runs even when the rest of the system is in a
    // broken state. We only recognize the zero-arg form plus `--help` /
    // `--version`; any other input is rejected with usage on stderr.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = args.first() {
        match first.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("aenv-rescue {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("aenv-rescue: unknown argument: {other}");
                eprintln!();
                eprint!("{HELP}");
                return ExitCode::from(2);
            }
        }
    }

    let aenv_home = aenv_home();
    let state_path = aenv_home.join("global-state.json");

    if !state_path.exists() {
        println!("No active global activation under {}.", aenv_home.display());
        return ExitCode::SUCCESS;
    }

    let body = match std::fs::read_to_string(&state_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("aenv-rescue: cannot read {}: {e}", state_path.display());
            return ExitCode::from(1);
        }
    };

    let state = match aenv_core::state::ActivationState::from_json(&body) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "aenv-rescue: state file at {} is malformed: {e}",
                state_path.display()
            );
            return ExitCode::from(1);
        }
    };

    let target_root = state.project_root.clone();
    println!(
        "Rescuing active global activation of '{}' under {}",
        state.active_namespace,
        target_root.display()
    );

    // 1. Remove every materialized file (symlink, regular file, or directory).
    for m in &state.managed_files {
        let full = target_root.join(&m.path);
        match std::fs::symlink_metadata(&full) {
            Ok(meta) => {
                let ft = meta.file_type();
                let kind = if ft.is_symlink() {
                    "symlink"
                } else if ft.is_dir() {
                    "directory"
                } else {
                    "file"
                };
                let removed = if ft.is_dir() && !ft.is_symlink() {
                    std::fs::remove_dir_all(&full)
                } else {
                    std::fs::remove_file(&full)
                };
                match removed {
                    Ok(()) => println!("  removed {kind} {}", full.display()),
                    Err(e) => {
                        eprintln!("  warning: could not remove {kind} {}: {e}", full.display())
                    }
                }
            }
            Err(_) => {
                // Already gone — nothing to do.
            }
        }
    }

    // 2. Restore every backup whose original slot was non-empty at activation.
    //    Schema v6+ tracks this via `was_present_before_activation` on the
    //    corresponding managed file; v1-5 default it to `true` on read,
    //    preserving the historical "always restore" semantics.
    for b in &state.backed_up {
        let original = target_root.join(&b.original_path);

        // Look up the matching managed file to decide whether the slot was
        // present pre-activation. If no match (defensive), assume present.
        let was_present = state
            .managed_files
            .iter()
            .find(|m| m.path == b.original_path)
            .map(|m| m.was_present_before_activation)
            .unwrap_or(true);

        if !was_present {
            continue;
        }

        if !b.backup_path.exists() {
            eprintln!(
                "  warning: backup {} missing; cannot restore {}",
                b.backup_path.display(),
                original.display()
            );
            continue;
        }
        if let Some(parent) = original.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if original.exists() {
            let _ = std::fs::remove_file(&original);
        }
        match std::fs::rename(&b.backup_path, &original) {
            Ok(()) => println!("  restored {}", original.display()),
            Err(e) => eprintln!("  warning: could not restore {}: {e}", original.display()),
        }
    }

    // 3. Tear down state + lock.
    let _ = std::fs::remove_file(&state_path);
    let _ = std::fs::remove_file(aenv_home.join("global.lock"));

    println!();
    println!("Rescue complete. Run `aenv global status` to confirm.");
    println!("Note: aenv-rescue did NOT run the namespace's on_deactivate hook.");
    println!("If the namespace's runtime needs uninstallation, do that manually.");
    ExitCode::SUCCESS
}
