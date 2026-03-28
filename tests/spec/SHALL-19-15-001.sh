# Test: SHALL-19-15-001
# Obligation: "The following 'special built-in' utilities shall be supported in
#   the shell command language. The output of each command, if any, shall be
#   written to standard output, subject to the normal redirection and piping
#   possible with all commands."
# Verifies: special built-ins exist and their output can be redirected.

# Verify key special built-ins exist by running them
: # colon — should succeed silently
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: : (colon) not available" >&2
    exit 1
fi

eval 'true'
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: eval not available" >&2
    exit 1
fi

export TEST_SBI_VAR=hello
if [ "$TEST_SBI_VAR" != "hello" ]; then
    printf '%s\n' "FAIL: export not working" >&2
    exit 1
fi
unset TEST_SBI_VAR

# Output can be redirected: export -p to a file
export REDIR_TEST=yes
export -p > "$TMPDIR/sbi_redir.txt"
if [ ! -s "$TMPDIR/sbi_redir.txt" ]; then
    printf '%s\n' "FAIL: export -p redirect produced empty file" >&2
    exit 1
fi
unset REDIR_TEST
rm -f "$TMPDIR/sbi_redir.txt"

# set outputs to stdout and can be piped
set_output=$(set)
if [ -z "$set_output" ]; then
    printf '%s\n' "FAIL: set produced no output" >&2
    exit 1
fi

exit 0
