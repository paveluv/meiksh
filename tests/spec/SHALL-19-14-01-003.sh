# Test: SHALL-19-14-01-003
# Obligation: "A <backslash> character that is not inside a bracket expression
#   shall preserve the literal value of the following character"
# Verifies: backslash outside brackets escapes special chars in expanded patterns.

# When a variable contains a pattern char, backslash-escaping makes it literal
star='*'
case '*' in
    "$star") ;; # double-quoting prevents glob, matches literal *
    *) printf '%s\n' "FAIL: quoted var with * did not match literal *" >&2; exit 1 ;;
esac

# In a case pattern, \* matches literal *
case '*' in \*) ;; *) printf '%s\n' "FAIL: \\* did not match literal *" >&2; exit 1 ;; esac
case 'a' in \*) printf '%s\n' "FAIL: \\* matched non-* char" >&2; exit 1 ;; *) ;; esac

exit 0
