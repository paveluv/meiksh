# Test: SHALL-19-14-01-015
# Obligation: "An <asterisk> is a pattern that shall match multiple characters,
#   as described in 2.14.2 Patterns Matching Multiple Characters."
# (Duplicate of SHALL-19-14-01-011)
# Verifies: * matches any string including empty.

case "" in *) ;; ?) printf '%s\n' "FAIL: * did not match empty" >&2; exit 1 ;; esac
case "abc" in a*c) ;; *) printf '%s\n' "FAIL: a*c did not match abc" >&2; exit 1 ;; esac
exit 0
