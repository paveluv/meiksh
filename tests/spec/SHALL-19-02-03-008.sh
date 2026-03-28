# Test: SHALL-19-02-03-008
# Obligation: (Duplicate of SHALL-19-02-03-003) "$" retains expansion meaning
#   inside double-quotes; $'...' does NOT trigger dollar-single-quote.
# Verifies: Same as SHALL-19-02-03-003.

X=test_val
r="$X"
[ "$r" = "test_val" ] || { printf '%s\n' "FAIL: param expansion" >&2; exit 1; }

# $' should not start dollar-single-quote inside double-quotes
r="$'x'"
case "$r" in
    *x*) ;;
    *)  printf '%s\n' "FAIL: \$' in dquotes unexpected result '$r'" >&2; exit 1 ;;
esac

exit 0
