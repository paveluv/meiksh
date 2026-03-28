# Test: SHALL-18-04-012
# Obligation: "If the requested action cannot be performed on an operand ...
#   the utility shall issue a diagnostic message to standard error and
#   continue processing the next operand in sequence, but the final exit
#   status shall be one that indicates an error occurred."
# Verifies: A failing operand produces stderr diagnostic and nonzero exit.

d="$TMPDIR/shall_18_04_012_$$"
mkdir -p "$d"

# cd to nonexistent should produce diagnostic on stderr
err=$(cd "$d/no_such_dir" 2>&1 >/dev/null)
rc=$?

if [ "$rc" = "0" ]; then
    printf '%s\n' "FAIL: cd to nonexistent dir should fail" >&2; exit 1
fi

if [ -z "$err" ]; then
    printf '%s\n' "FAIL: cd to nonexistent dir should produce stderr diagnostic" >&2
    exit 1
fi

rm -rf "$d"
exit 0
