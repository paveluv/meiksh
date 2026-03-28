# Test: SHALL-19-14-01-011
# Obligation: "An <asterisk> is a pattern that shall match multiple characters,
#   as described in 2.14.2 Patterns Matching Multiple Characters."
# Verifies: * matches any string including empty.

case "" in *) ;; ?) printf '%s\n' "FAIL: * did not match empty" >&2; exit 1 ;; esac
case "hello" in *) ;; ?) printf '%s\n' "FAIL: * did not match hello" >&2; exit 1 ;; esac
case "abc" in a*) ;; *) printf '%s\n' "FAIL: a* did not match abc" >&2; exit 1 ;; esac
case "abc" in *c) ;; *) printf '%s\n' "FAIL: *c did not match abc" >&2; exit 1 ;; esac
case "abc" in a*c) ;; *) printf '%s\n' "FAIL: a*c did not match abc" >&2; exit 1 ;; esac

exit 0
