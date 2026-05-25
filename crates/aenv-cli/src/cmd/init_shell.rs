//! `aenv init-shell <bash|zsh|fish>` — print a shell hook script for
//! sourcing in the user's rc file. The hook calls `aenv activate-if-needed`
//! on every directory change so the right namespace is auto-activated as
//! the user `cd`s between projects.

use aenv_core::error::{AenvError, Result};

const BASH_HOOK: &str = include_str!("../shell/bash.sh");
const ZSH_HOOK: &str = include_str!("../shell/zsh.sh");
const FISH_HOOK: &str = include_str!("../shell/fish.sh");

pub fn run(shell: &str) -> Result<()> {
    let script = match shell {
        "bash" => BASH_HOOK,
        "zsh" => ZSH_HOOK,
        "fish" => FISH_HOOK,
        other => {
            return Err(AenvError::ManifestInvalid(format!(
                "unknown shell '{other}'; supported: bash, zsh, fish"
            )));
        }
    };
    print!("{script}");
    Ok(())
}
