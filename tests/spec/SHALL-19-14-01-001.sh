# Test: SHALL-19-14-01-001
# Obligation: "The following patterns shall match a single character: ordinary
#   characters, special pattern characters, and pattern bracket expressions."
# Verifies: ordinary char, ?, and [...] each match exactly one character.

# Ordinary character matches itself
case "a" in a) ;; *) printf '%s\n' "FAIL: ordinary 'a' did not match 'a'" >&2; exit 1 ;; esac

# ? matches any single character
case "x" in ?) ;; *) printf '%s\n' "FAIL: ? did not match single char" >&2; exit 1 ;; esac
case "xy" in ?) printf '%s\n' "FAIL: ? matched two chars" >&2; exit 1 ;; *) ;; esac

# Bracket expression matches one character
case "b" in [abc]) ;; *) printf '%s\n' "FAIL: [abc] did not match 'b'" >&2; exit 1 ;; esac
case "d" in [abc]) printf '%s\n' "FAIL: [abc] matched 'd'" >&2; exit 1 ;; *) ;; esac

exit 0
