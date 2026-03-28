# Test: SHALL-19-24-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

err=$(readonly RO_STDERR_TEST=ok 2>&1 >/dev/null)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid readonly produced stderr: $err" >&2
    exit 1
fi

exit 0
