# Test: SHALL-19-14-03-005
# Obligation: "The <slash> character in a pathname shall be explicitly matched"
# (Duplicate of SHALL-19-14-03-001)
# Verifies: glob * and ? do not match / in pathnames.

# In case patterns (not pathname expansion), ? and * match any chars including /
case "a/b" in a?b) ;; *) printf '%s\n' "FAIL: ? should match / in case pattern" >&2; exit 1 ;; esac
case "a/b" in a*b) ;; *) printf '%s\n' "FAIL: * should match / in case pattern" >&2; exit 1 ;; esac
exit 0
