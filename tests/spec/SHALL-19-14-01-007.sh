# Test: SHALL-19-14-01-007
# Obligation: "When unquoted, unescaped, and not inside a bracket expression,
#   the following three characters shall have special meaning in the
#   specification of patterns"
# Verifies: ?, *, [ are special when unquoted/unescaped/outside brackets.

# ? is special (matches any single char)
case "x" in ?) ;; *) printf '%s\n' "FAIL: ? not special" >&2; exit 1 ;; esac

# * is special (matches any string)
case "hello" in *) ;; ?) printf '%s\n' "FAIL: * not special" >&2; exit 1 ;; esac

# [ introduces bracket expression
case "a" in [abc]) ;; *) printf '%s\n' "FAIL: [ not special" >&2; exit 1 ;; esac

# When quoted, they lose special meaning
case "?" in '?') ;; *) printf '%s\n' "FAIL: quoted ? still special" >&2; exit 1 ;; esac
case "hello" in '*') printf '%s\n' "FAIL: quoted * matched multi-char" >&2; exit 1 ;; *) ;; esac

exit 0
