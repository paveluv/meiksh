# Test: SHALL-19-10-02-006
# Obligation: "[case only] When the TOKEN is exactly the reserved word in, the
#   token identifier for in shall result. ... [for only] When the TOKEN is
#   exactly the reserved word in or do, the token identifier for in or do shall
#   result"
# Verifies: 'in' recognized after case word; 'in' and 'do' after for name.

# 'in' after case word
result=""
case x in
    x) result="ok" ;;
esac
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: 'in' not recognized after case word" >&2
    exit 1
fi

# 'in' after for name
result=""
for i in a; do result="$i"; done
if [ "$result" != "a" ]; then
    printf '%s\n' "FAIL: 'in' not recognized after for name" >&2
    exit 1
fi

# 'do' after for name (no 'in' clause)
test_do() {
    result=""
    for i do result="${result}${i}"; done
    printf '%s' "$result"
}
out=$(test_do x y)
if [ "$out" != "xy" ]; then
    printf '%s\n' "FAIL: 'do' not recognized after for name" >&2
    exit 1
fi

exit 0
