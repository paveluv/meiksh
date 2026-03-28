# Test: SHALL-19-18-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# Valid continue produces no stderr
err=$(for i in 1; do continue; done 2>&1 >/dev/null)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid continue produced stderr output: $err" >&2
    exit 1
fi

exit 0
