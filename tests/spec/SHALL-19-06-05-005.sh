# Test: SHALL-19-06-05-005
# Obligation: "Fields which contain no results from expansions shall not be
#   affected by field splitting, and shall remain unaltered."
# Verifies: literal fields pass through splitting unchanged.

count_args() { printf '%s\n' "$#"; }

# Literal words are not split even with unusual IFS
IFS=o
n=$(count_args hello world)
if [ "$n" != "2" ]; then
    printf '%s\n' "FAIL: literal words were split by IFS=o: got $n fields" >&2
    IFS=' '
    exit 1
fi

IFS=' '
exit 0
