# Test: SHALL-19-09-001
# Obligation: "Unless otherwise stated, the exit status of a command shall be
#   that of the last simple command executed by the command. There shall be
#   no limit on the size of any shell command other than that imposed by the
#   underlying system."
# Verifies: compound command exit status is from last simple command.

# Brace group: exit status of last command
"$SHELL" -c '{ true; false; }'; s1=$?
if [ "$s1" -ne 1 ]; then
    printf '%s\n' "FAIL: brace group exit status should be 1 (from false)" >&2
    exit 1
fi

"$SHELL" -c '{ false; true; }'; s2=$?
if [ "$s2" -ne 0 ]; then
    printf '%s\n' "FAIL: brace group exit status should be 0 (from true)" >&2
    exit 1
fi

# if/else: exit status of branch taken
"$SHELL" -c 'if true; then exit 5; fi'; s3=$?
if [ "$s3" -ne 5 ]; then
    printf '%s\n' "FAIL: if-then exit status should be 5" >&2
    exit 1
fi

# for loop: exit status of last iteration
result=$("$SHELL" -c 'for i in 1 2 3; do true; done; exit $?')
"$SHELL" -c 'for i in 1 2 3; do true; done'; s4=$?
if [ "$s4" -ne 0 ]; then
    printf '%s\n' "FAIL: for loop exit status should be 0" >&2
    exit 1
fi

exit 0
