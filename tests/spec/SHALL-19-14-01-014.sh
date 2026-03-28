# Test: SHALL-19-14-01-014
# Obligation: "A <question-mark> is a pattern that shall match any character."
# (Duplicate of SHALL-19-14-01-009)
# Verifies: ? matches any single character.

case "x" in ?) ;; *) printf '%s\n' "FAIL: ? did not match x" >&2; exit 1 ;; esac
case "" in ?) printf '%s\n' "FAIL: ? matched empty" >&2; exit 1 ;; *) ;; esac
exit 0
