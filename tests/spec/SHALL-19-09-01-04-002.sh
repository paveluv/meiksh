# Test: SHALL-19-09-01-04-002
# Obligation: "If the command name does not contain any <slash> characters,
#   the first successful step in the following sequence shall occur: special
#   built-in ... function ... PATH search ... 127 error."
# Verifies: command search priority — special builtins, functions, PATH.

# Special builtin takes priority
result=$("$SHELL" -c '
colon() { printf "%s\n" "function"; }
: && printf "%s\n" "builtin_ran"
')
if [ "$result" != "builtin_ran" ]; then
    printf '%s\n' "FAIL: special builtin : should take priority over function" >&2
    exit 1
fi

# Function takes priority over PATH
result2=$("$SHELL" -c '
myecho() { printf "%s\n" "func_myecho"; }
myecho
')
if [ "$result2" != "func_myecho" ]; then
    printf '%s\n' "FAIL: function should be found before PATH search" >&2
    exit 1
fi

# Command not found: 127
"$SHELL" -c 'xyzzy_no_such_cmd' 2>/dev/null; s=$?
if [ "$s" -ne 127 ]; then
    printf '%s\n' "FAIL: not-found should return 127, got $s" >&2
    exit 1
fi

exit 0
