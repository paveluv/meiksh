# Test: SHALL-19-02-03-012
# Obligation: "The parameter '@' has special meaning inside double-quotes ...
#   described in 2.5.2 Special Parameters."
# Verifies: "$@" expands to separate words, one per positional parameter.

set -- "a b" "c d" "e"

count=0
for arg in "$@"; do
    count=$(( count + 1 ))
done
[ "$count" = "3" ] || { printf '%s\n' "FAIL: \"\$@\" should produce 3 words, got $count" >&2; exit 1; }

# First word should contain space
set -- "a b" "c d" "e"
first=
for arg in "$@"; do
    first="$arg"
    break
done
[ "$first" = "a b" ] || { printf '%s\n' "FAIL: first word should be 'a b'" >&2; exit 1; }

# With no positional params, "$@" produces zero words
set --
count=0
for arg in "$@"; do
    count=$(( count + 1 ))
done
[ "$count" = "0" ] || { printf '%s\n' "FAIL: \"\$@\" with no params should be 0 words" >&2; exit 1; }

exit 0
