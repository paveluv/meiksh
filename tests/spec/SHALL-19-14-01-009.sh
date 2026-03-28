# Test: SHALL-19-14-01-009
# Obligation: "A <question-mark> is a pattern that shall match any character."
# Verifies: ? matches exactly one arbitrary character.

case "a" in ?) ;; *) printf '%s\n' "FAIL: ? did not match 'a'" >&2; exit 1 ;; esac
case "Z" in ?) ;; *) printf '%s\n' "FAIL: ? did not match 'Z'" >&2; exit 1 ;; esac
case "9" in ?) ;; *) printf '%s\n' "FAIL: ? did not match '9'" >&2; exit 1 ;; esac
case "" in ?) printf '%s\n' "FAIL: ? matched empty string" >&2; exit 1 ;; *) ;; esac
case "ab" in ?) printf '%s\n' "FAIL: ? matched two chars" >&2; exit 1 ;; *) ;; esac

# ? in a pattern context
case "abc" in a?c) ;; *) printf '%s\n' "FAIL: a?c did not match abc" >&2; exit 1 ;; esac
case "aXc" in a?c) ;; *) printf '%s\n' "FAIL: a?c did not match aXc" >&2; exit 1 ;; esac

exit 0
