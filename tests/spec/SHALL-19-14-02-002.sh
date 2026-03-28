# Test: SHALL-19-14-02-002
# Obligation: "The concatenation of patterns matching a single character is a
#   valid pattern that shall match the concatenation of the single characters
#   or collating elements matched by each of the concatenated patterns."
# Verifies: concatenated single-char patterns match concatenated characters.

# a + ? + c = a?c matches any 3-char string starting with a ending with c
case "abc" in a?c) ;; *) printf '%s\n' "FAIL: a?c did not match abc" >&2; exit 1 ;; esac
case "axc" in a?c) ;; *) printf '%s\n' "FAIL: a?c did not match axc" >&2; exit 1 ;; esac
case "ac" in a?c) printf '%s\n' "FAIL: a?c matched ac (2 chars)" >&2; exit 1 ;; *) ;; esac

# Multiple ? = exact length match
case "ab" in ??) ;; *) printf '%s\n' "FAIL: ?? did not match ab" >&2; exit 1 ;; esac
case "a" in ??) printf '%s\n' "FAIL: ?? matched single char" >&2; exit 1 ;; *) ;; esac
case "abc" in ??) printf '%s\n' "FAIL: ?? matched 3 chars" >&2; exit 1 ;; *) ;; esac

# Bracket + ordinary
case "a1" in [abc][0-9]) ;; *) printf '%s\n' "FAIL: [abc][0-9] did not match a1" >&2; exit 1 ;; esac

exit 0
