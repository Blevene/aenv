# aenv zsh shell hook.
#
# Sources this script in ~/.zshrc to enable cd-based auto-activation:
#   eval "$(aenv init-shell zsh)"
#
# On every chpwd (directory change), the hook calls `aenv activate-if-needed`,
# which walks the cwd's ancestors looking for the nearest `.aenv` pin and
# transitions the project's active namespace if it differs from what the
# previous invocation activated. Tracked via the _AENV_ACTIVE env var.
#
# To uninstall: remove the eval line from ~/.zshrc and start a new shell.

_aenv_chpwd() {
    local new_active
    new_active="$(command aenv activate-if-needed "${_AENV_ACTIVE:-}" 2>/dev/null)" || return
    export _AENV_ACTIVE="$new_active"
}

# Use ZSH's chpwd_functions array so we compose with other tools.
typeset -gaU chpwd_functions
chpwd_functions+=(_aenv_chpwd)

# Run once at shell startup so the initial cwd is reconciled.
_aenv_chpwd
