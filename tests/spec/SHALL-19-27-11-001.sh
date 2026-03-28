# Test: SHALL-19-27-11-001
# Obligation: "The standard error shall be used only for diagnostic messages
#   and the warning message specified in EXIT STATUS."

# Valid shift produces no stderr
set -- a b c
err=$(shift 2>&1 >/dev/null)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid shift produced stderr: $err" >&2
    exit 1
fi

exit 0
