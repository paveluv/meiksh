# Test: SHALL-19-20-14-001
# Obligation: "If there are no arguments, or only null arguments, eval shall
#   return a zero exit status; otherwise, it shall return the exit status of
#   the command defined by the string of concatenated arguments."

# No arguments returns 0
eval
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: eval with no args did not return 0" >&2
    exit 1
fi

# Only null arguments returns 0
eval "" ""
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: eval with null args did not return 0" >&2
    exit 1
fi

# Returns exit status of executed command
eval 'true'
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: eval true did not return 0" >&2
    exit 1
fi

eval 'false'
if [ $? -eq 0 ]; then
    printf '%s\n' "FAIL: eval false returned 0" >&2
    exit 1
fi

exit 0
