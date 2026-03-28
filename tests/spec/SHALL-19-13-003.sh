# Test: SHALL-19-13-003
# Obligation: "Variables with the export attribute, along with those explicitly
#   exported for the duration of the command, shall be passed to the utility
#   environment variables"
# Verifies: exported variables are visible to child; unexported are not.

EXPORTED_VAR=hello
export EXPORTED_VAR
UNEXPORTED_VAR=secret

out=$(sh -c 'printf "%s\n" "$EXPORTED_VAR"')
if [ "$out" != "hello" ]; then
    printf '%s\n' "FAIL: exported var not passed to child: got [$out]" >&2
    exit 1
fi

out=$(sh -c 'printf "%s\n" "$UNEXPORTED_VAR"')
if [ -n "$out" ]; then
    printf '%s\n' "FAIL: unexported var leaked to child: got [$out]" >&2
    exit 1
fi

# Prefix assignment exports for duration of command
out=$(TEMP_VAR=prefix sh -c 'printf "%s\n" "$TEMP_VAR"')
if [ "$out" != "prefix" ]; then
    printf '%s\n' "FAIL: prefix assignment not passed to child: got [$out]" >&2
    exit 1
fi
if [ -n "$TEMP_VAR" ]; then
    printf '%s\n' "FAIL: prefix assignment persisted in parent" >&2
    exit 1
fi
exit 0
