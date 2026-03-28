# Test: SHALL-19-14-01-016
# Obligation: "A <left-square-bracket> shall introduce a bracket expression...
#   the <exclamation-mark> character ('!') shall replace the <circumflex>...
#   A <left-square-bracket> that does not introduce a valid bracket expression
#   shall match the character itself."
# (Duplicate of SHALL-19-14-01-013)
# Verifies: bracket expression with ! negation; invalid [ is literal.

case "d" in [!abc]) ;; *) printf '%s\n' "FAIL: [!abc] did not match d" >&2; exit 1 ;; esac
case "a" in [!abc]) printf '%s\n' "FAIL: [!abc] matched a" >&2; exit 1 ;; *) ;; esac
case "[" in [) ;; *) printf '%s\n' "FAIL: invalid [ not literal" >&2; exit 1 ;; esac
exit 0
