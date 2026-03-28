# Test: SHALL-19-14-02-006
# Obligation: "each <asterisk> shall match a string of zero or more characters,
#   matching the greatest possible number of characters"
# (Duplicate of SHALL-19-14-02-003)
# Verifies: greedy * matching in patterns.

case "aXXbYYb" in a*b) ;; *) printf '%s\n' "FAIL: a*b no match aXXbYYb" >&2; exit 1 ;; esac
var="a.b.c"
r="${var##*.}"
if [ "$r" != "c" ]; then printf '%s\n' "FAIL: greedy ## gave [$r]" >&2; exit 1; fi
r="${var#*.}"
if [ "$r" != "b.c" ]; then printf '%s\n' "FAIL: non-greedy # gave [$r]" >&2; exit 1; fi
exit 0
