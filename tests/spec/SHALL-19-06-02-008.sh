# Test: SHALL-19-06-02-008
# Obligation: "Indicate Error if Null or Unset. If parameter is unset or null,
#   the expansion of word [...] shall be written to standard error and the
#   shell exits with a non-zero exit status."
# Verifies: ${param:?word} writes error and exits for unset/null.

# Unset parameter: should fail
unset evar
msg=$(eval '${evar:?custom error}' 2>&1) && {
    printf '%s\n' "FAIL: \${evar:?...} did not exit non-zero" >&2
    exit 1
}

# Null parameter: should also fail
evar2=""
msg2=$(eval '${evar2:?null error}' 2>&1) && {
    printf '%s\n' "FAIL: \${evar2:?...} (null) did not exit non-zero" >&2
    exit 1
}

# Set parameter: should succeed
evar3="ok"
result=$(eval 'printf "%s\n" "${evar3:?should not fire}"' 2>&1)
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: \${evar3:?...} (set) gave '$result', expected 'ok'" >&2
    exit 1
fi

exit 0
