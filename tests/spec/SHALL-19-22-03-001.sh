# Test: SHALL-19-22-03-001
# Obligation: "The exit utility shall cause the shell to exit from its current
#   execution environment. If the current execution environment is a subshell
#   environment, the shell shall exit from the subshell environment."

# exit in subshell exits only the subshell
(exit 0)
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: exit 0 in subshell did not return 0" >&2
    exit 1
fi

(exit 42)
st=$?
if [ "$st" -ne 42 ]; then
    printf '%s\n' "FAIL: exit 42 in subshell returned $st instead of 42" >&2
    exit 1
fi

# Parent shell continues after subshell exit
after=yes
if [ "$after" != "yes" ]; then
    printf '%s\n' "FAIL: parent did not continue after subshell exit" >&2
    exit 1
fi

exit 0
