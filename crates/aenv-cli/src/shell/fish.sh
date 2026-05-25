# aenv fish shell hook.
#
# Sources this script in ~/.config/fish/config.fish to enable cd-based
# auto-activation:
#   aenv init-shell fish | source
#
# Fish has a first-class `--on-variable PWD` event, which fires on every
# directory change.
#
# To uninstall: remove the `aenv init-shell fish | source` line and start
# a new shell.

function _aenv_chpwd --on-variable PWD
    set -gx _AENV_ACTIVE (command aenv activate-if-needed "$_AENV_ACTIVE" 2>/dev/null)
end

# Run once at shell startup so the initial cwd is reconciled.
_aenv_chpwd
