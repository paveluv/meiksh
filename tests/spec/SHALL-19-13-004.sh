# Test: SHALL-19-13-004
# Obligation: "The environment of the shell process shall not be changed by the
#   utility unless explicitly specified by the utility description (for example,
#   cd and umask)."
# Verifies: external utility cannot change parent shell's cwd or variables.

orig_dir=$(pwd)
sh -c 'cd /tmp'
if [ "$(pwd)" != "$orig_dir" ]; then
    printf '%s\n' "FAIL: external cd changed parent cwd" >&2
    exit 1
fi

PARENT_VAR=before
sh -c 'PARENT_VAR=after'
if [ "$PARENT_VAR" != "before" ]; then
    printf '%s\n' "FAIL: external cmd changed parent variable" >&2
    exit 1
fi
exit 0
