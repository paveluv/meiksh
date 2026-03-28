# Test: SHALL-19-14-01-008
# Obligation: "When unquoted, unescaped, and not inside a bracket expression,
#   the following three characters shall have special meaning: ?"
# Verifies: ? is a special pattern character.

case "x" in ?) ;; *) printf '%s\n' "FAIL: ? did not match single char" >&2; exit 1 ;; esac
case "xy" in ?) printf '%s\n' "FAIL: ? matched two chars" >&2; exit 1 ;; *) ;; esac
exit 0
