# Cargonaut shell wrapper — bash / zsh.
#
# Source this file (e.g. add `source /path/to/contrib/cargonaut.sh` to
# your ~/.bashrc or ~/.zshrc) to define the `cargonaut` shell function.
# The function writes a temp file path into $CARGONAUT_EXIT_CWD_FILE
# before invoking the binary; after cargonaut exits, it `cd`s into the
# directory the binary wrote there (the active pane's cwd at quit time,
# per FR-017).
#
# Behavior when invoked without this wrapper: the binary just runs
# normally and your shell stays in whatever directory you launched it
# from. The wrapper is opt-in.
#
# Override the binary location with $CARGONAUT_BIN if not on PATH:
#   CARGONAUT_BIN=/path/to/cargonaut cargonaut ~/work /tmp

cargonaut() {
    local bin="${CARGONAUT_BIN:-cargonaut}"
    local exit_cwd_file
    exit_cwd_file="$(mktemp -t cargonaut-exit-cwd.XXXXXX)" || {
        printf 'cargonaut wrapper: mktemp failed; running without cd-on-exit\n' >&2
        command "${bin}" "$@"
        return $?
    }
    CARGONAUT_EXIT_CWD_FILE="${exit_cwd_file}" command "${bin}" "$@"
    local rc=$?
    if [ -s "${exit_cwd_file}" ]; then
        local target
        target="$(cat "${exit_cwd_file}")"
        if [ -d "${target}" ]; then
            cd "${target}" || true
        fi
    fi
    rm -f "${exit_cwd_file}"
    return ${rc}
}
