# Test: SHALL-19-17-04-002
# Obligation: "Implementations shall not support any options."

# Arguments starting with - are not options, just ignored arguments
: -x -h --help
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: colon with -x -h --help did not return 0" >&2
    exit 1
fi

exit 0
