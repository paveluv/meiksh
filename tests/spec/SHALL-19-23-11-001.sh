# Test: SHALL-19-23-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# Valid export produces no stderr
err=$(export EXPORT_STDERR_TEST=ok 2>&1 >/dev/null)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid export produced stderr: $err" >&2
    exit 1
fi

exit 0
