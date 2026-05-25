# aenv bash shell hook.
#
# Sources this script in ~/.bashrc to enable cd-based auto-activation:
#   eval "$(aenv init-shell bash)"
#
# Bash has no first-class chpwd hook, so this drives off PROMPT_COMMAND and
# only does work when $PWD has actually changed since the last fire (the
# guard avoids a roundtrip on every prompt redraw).
#
# To uninstall: remove the eval line from ~/.bashrc and start a new shell.

_aenv_chpwd() {
    if [ "${_AENV_LAST_PWD:-}" = "$PWD" ]; then
        return
    fi
    _AENV_LAST_PWD="$PWD"
    _AENV_ACTIVE="$(command aenv activate-if-needed "${_AENV_ACTIVE:-}" 2>/dev/null)"
    export _AENV_ACTIVE
}

# Compose with any existing PROMPT_COMMAND. Run our hook first, then theirs.
case "${PROMPT_COMMAND:-}" in
    *_aenv_chpwd*) ;;
    "") PROMPT_COMMAND="_aenv_chpwd" ;;
    *)  PROMPT_COMMAND="_aenv_chpwd; ${PROMPT_COMMAND}" ;;
esac

# Run once at shell startup so the initial cwd is reconciled.
_aenv_chpwd
