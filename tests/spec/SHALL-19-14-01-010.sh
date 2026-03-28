# Test: SHALL-19-14-01-010
# Obligation: "When unquoted, unescaped, and not inside a bracket expression,
#   the following three characters shall have special meaning: *"
# Verifies: * is a special pattern character.

case "hello" in *) ;; ?) printf '%s\n' "FAIL: * not special" >&2; exit 1 ;; esac
case "" in *) ;; ?) printf '%s\n' "FAIL: * did not match empty" >&2; exit 1 ;; esac
exit 0
