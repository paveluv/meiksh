# Test: SHALL-19-14-01-012
# Obligation: "When unquoted, unescaped, and not inside a bracket expression,
#   the following three characters shall have special meaning: ["
# Verifies: [ introduces a bracket expression.

case "a" in [abc]) ;; *) printf '%s\n' "FAIL: [ did not introduce bracket expr" >&2; exit 1 ;; esac
case "d" in [abc]) printf '%s\n' "FAIL: [abc] matched d" >&2; exit 1 ;; *) ;; esac
exit 0
