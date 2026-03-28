# Test: SHALL-19-16-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# break with valid usage produces no stderr
err=$(for i in 1; do break; done 2>&1 >/dev/null)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid break produced stderr output: $err" >&2
    exit 1
fi

exit 0
