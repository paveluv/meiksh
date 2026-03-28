# Test: SHALL-19-14-01-004
# Obligation: "A <backslash> character that is not inside a bracket expression
#   shall preserve the literal value of the following character"
# (Duplicate of SHALL-19-14-01-003)
# Verifies: backslash outside brackets preserves literal value.

case '*' in \*) ;; *) printf '%s\n' "FAIL: \\* did not match literal *" >&2; exit 1 ;; esac
case 'a' in \*) printf '%s\n' "FAIL: \\* matched non-*" >&2; exit 1 ;; *) ;; esac
exit 0
