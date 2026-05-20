# Cargonaut shell wrapper — fish.
#
# Source this file (e.g. add `source /path/to/contrib/cargonaut.fish` to
# your config.fish) to define the `cargonaut` fish function. The function
# writes a temp file path into $CARGONAUT_EXIT_CWD_FILE before invoking
# the binary; after cargonaut exits, it cd's into the directory the
# binary wrote there (the active pane's cwd at quit time, per FR-017).
#
# Override the binary location with $CARGONAUT_BIN if not on PATH:
#   CARGONAUT_BIN=/path/to/cargonaut cargonaut ~/work /tmp

function cargonaut
    set -l bin (set -q CARGONAUT_BIN; and echo $CARGONAUT_BIN; or echo cargonaut)
    set -l exit_cwd_file (mktemp -t cargonaut-exit-cwd.XXXXXX)
    or begin
        echo "cargonaut wrapper: mktemp failed; running without cd-on-exit" >&2
        command $bin $argv
        return $status
    end
    CARGONAUT_EXIT_CWD_FILE=$exit_cwd_file command $bin $argv
    set -l rc $status
    if test -s $exit_cwd_file
        set -l target (cat $exit_cwd_file)
        if test -d "$target"
            cd "$target"
        end
    end
    rm -f $exit_cwd_file
    return $rc
end
