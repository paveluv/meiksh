# Test: SHALL-19-06-011
# Obligation: "When expanding words for a command about to be executed, and the
#   word will be the command name or an argument to the command, the expansions
#   shall be carried out in the current shell execution environment."
# Verifies: expansion side effects in command args persist in current shell.

unset sidevar
printf '%s\n' "${sidevar:=hello}" >/dev/null
if [ "$sidevar" != "hello" ]; then
    printf '%s\n' "FAIL: \${sidevar:=hello} in command arg did not persist" >&2
    exit 1
fi

exit 0
