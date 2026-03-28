# Test: SHALL-19-09-02-003
# Obligation: "The standard input, standard output, or both of a command shall
#   be considered to be assigned by the pipeline before any redirection specified
#   by redirection operators that are part of the command"
# Verifies: Explicit redirection overrides pipe assignment.

tmpf="$TMPDIR/shall-19-09-02-003.$$"
trap 'rm -f "$tmpf"' EXIT

# cmd1 >file | cmd2: stdout of cmd1 goes to file, not pipe
printf '%s\n' "to_file" >"$tmpf" | cat >/dev/null
content=$(cat "$tmpf")
if [ "$content" != "to_file" ]; then
    printf '%s\n' "FAIL: redirection did not override pipe on left side" >&2
    exit 1
fi

# cmd1 | cmd2 >file: stdout of cmd2 goes to file, not terminal
printf '%s\n' "piped" | cat >"$tmpf"
content=$(cat "$tmpf")
if [ "$content" != "piped" ]; then
    printf '%s\n' "FAIL: redirection did not override pipe on right side" >&2
    exit 1
fi

exit 0
