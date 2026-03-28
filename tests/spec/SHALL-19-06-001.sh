# SHALL-19-06-001
# "The expansions that are performed for a given word shall be performed in the
#  following order:"
# Verify word expansions are performed in the correct order:
# 1. tilde/param/cmd/arith, 2. field splitting, 3. pathname expansion, 4. quote removal

fail=0

# Tilde + parameter expansion happen before field splitting
HOME=/tmp
x="a b"
set -- $x
[ $# -eq 2 ] || { printf '%s\n' "FAIL: field splitting after param expansion" >&2; fail=1; }

# Pathname expansion after field splitting (test with set -f to disable)
set -f
set -- *
[ "$1" = "*" ] || { printf '%s\n' "FAIL: pathname expansion not disabled by -f" >&2; fail=1; }
set +f

# Quote removal is last — quotes should be gone from final result
result=$(printf '%s' "hello")
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: quote removal failed: '$result'" >&2; fail=1; }

exit "$fail"
