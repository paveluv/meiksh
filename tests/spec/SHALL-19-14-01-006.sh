# Test: SHALL-19-14-01-006
# Obligation: "An ordinary character is a pattern that shall match itself.
#   ...Matching shall be based on the bit pattern used for encoding the
#   character, not on the graphic representation of the character. If any
#   character (ordinary, shell special, or pattern special) is quoted, or
#   escaped with a <backslash>, that pattern shall match the character itself."
# Verifies: ordinary chars match themselves; quoted specials match literally.

# Ordinary characters match themselves
case "Z" in Z) ;; *) printf '%s\n' "FAIL: Z did not match Z" >&2; exit 1 ;; esac
case "5" in 5) ;; *) printf '%s\n' "FAIL: 5 did not match 5" >&2; exit 1 ;; esac

# Quoted special characters match themselves
case '*' in '*') ;; *) printf '%s\n' "FAIL: quoted * did not match literal *" >&2; exit 1 ;; esac
case '?' in '?') ;; *) printf '%s\n' "FAIL: quoted ? did not match literal ?" >&2; exit 1 ;; esac
case '[' in '[') ;; *) printf '%s\n' "FAIL: quoted [ did not match literal [" >&2; exit 1 ;; esac

# Escaped special characters match themselves
case '|' in \|) ;; *) printf '%s\n' "FAIL: \\| did not match literal |" >&2; exit 1 ;; esac

exit 0
