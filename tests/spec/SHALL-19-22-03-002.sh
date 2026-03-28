# Test: SHALL-19-22-03-002
# Obligation: "If n is specified and has a value between 0 and 255 inclusive,
#   the wait status of the shell or subshell shall indicate that it exited
#   with exit status n."

# exit with values 0-255
(exit 0)
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: exit 0 did not produce status 0" >&2
    exit 1
fi

(exit 1)
if [ $? -ne 1 ]; then
    printf '%s\n' "FAIL: exit 1 did not produce status 1" >&2
    exit 1
fi

(exit 127)
if [ $? -ne 127 ]; then
    printf '%s\n' "FAIL: exit 127 did not produce status 127" >&2
    exit 1
fi

(exit 255)
if [ $? -ne 255 ]; then
    printf '%s\n' "FAIL: exit 255 did not produce status 255" >&2
    exit 1
fi

exit 0
