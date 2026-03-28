# Test: SHALL-19-23-03-001
# Obligation: "The shell shall give the export attribute to the variables
#   corresponding to the specified names, which shall cause them to be in the
#   environment of subsequently executed commands."

# export makes variable available to child processes
EXPORT_TEST1=value1
export EXPORT_TEST1
result=$(printf '%s' "$EXPORT_TEST1")
if [ "$result" != "value1" ]; then
    printf '%s\n' "FAIL: exported variable not visible in subshell" >&2
    exit 1
fi

# export with name=value sets and exports
export EXPORT_TEST2=value2
result=$(printf '%s' "$EXPORT_TEST2")
if [ "$result" != "value2" ]; then
    printf '%s\n' "FAIL: export name=value did not work" >&2
    exit 1
fi

exit 0
