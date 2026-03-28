# Test: SHALL-19-14-01-013
# Obligation: "A <left-square-bracket> shall introduce a bracket expression
#   ...the <exclamation-mark> character ('!') shall replace the <circumflex>
#   character ('^') in its role in a non-matching list... A <left-square-bracket>
#   that does not introduce a valid bracket expression shall match the character
#   itself."
# Verifies: bracket expressions, ! for negation, invalid [ matches literally.

# Basic bracket expression
case "b" in [abc]) ;; *) printf '%s\n' "FAIL: [abc] did not match b" >&2; exit 1 ;; esac
case "d" in [abc]) printf '%s\n' "FAIL: [abc] matched d" >&2; exit 1 ;; *) ;; esac

# Negation with !
case "d" in [!abc]) ;; *) printf '%s\n' "FAIL: [!abc] did not match d" >&2; exit 1 ;; esac
case "a" in [!abc]) printf '%s\n' "FAIL: [!abc] matched a" >&2; exit 1 ;; *) ;; esac

# Range expression
case "m" in [a-z]) ;; *) printf '%s\n' "FAIL: [a-z] did not match m" >&2; exit 1 ;; esac

# Invalid bracket (no closing ]) matches literal [
case "[" in [) ;; *) printf '%s\n' "FAIL: lone [ did not match literal [" >&2; exit 1 ;; esac

exit 0
