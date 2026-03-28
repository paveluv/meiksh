# Test: SHALL-19-14-02-005
# Obligation: "The concatenation of patterns matching a single character is a
#   valid pattern that shall match the concatenation of the single characters"
# (Duplicate of SHALL-19-14-02-002)
# Verifies: concatenated single-char patterns match concatenated chars.

case "abc" in a?c) ;; *) printf '%s\n' "FAIL: a?c no match abc" >&2; exit 1 ;; esac
case "ab" in ??) ;; *) printf '%s\n' "FAIL: ?? no match ab" >&2; exit 1 ;; esac
case "a" in ??) printf '%s\n' "FAIL: ?? matched 1 char" >&2; exit 1 ;; *) ;; esac
exit 0
