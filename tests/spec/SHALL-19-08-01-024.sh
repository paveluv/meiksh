# Test: SHALL-19-08-01-024
# Obligation: "The shell is not required to write a diagnostic message, but
#   the utility itself shall write a diagnostic message if required to do so."
# Verifies: utility writes its own diagnostic when encountering an error;
#   shell does not need to add its own diagnostic on top.

# ls on a nonexistent path should produce a diagnostic from ls, not the shell
err=$("$SHELL" -c 'ls /no_such_path_xyzzy_test' 2>&1 >/dev/null)
if [ -z "$err" ]; then
    printf '%s\n' "FAIL: utility did not write diagnostic on error" >&2
    exit 1
fi

exit 0
