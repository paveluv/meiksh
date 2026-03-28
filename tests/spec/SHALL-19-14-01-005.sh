# Test: SHALL-19-14-01-005
# Obligation: "All of the requirements and effects of quoting on ordinary,
#   shell special, and special pattern characters shall apply to escaping in
#   this context"
# Verifies: quoting/escaping rules apply uniformly to pattern characters.

# Escaped special chars in expanded patterns match literally
case '*' in \*) ;; *) printf '%s\n' "FAIL: escaped * not literal" >&2; exit 1 ;; esac
case '?' in \?) ;; *) printf '%s\n' "FAIL: escaped ? not literal" >&2; exit 1 ;; esac
case '[' in \[) ;; *) printf '%s\n' "FAIL: escaped [ not literal" >&2; exit 1 ;; esac

# Single-quoted special chars match literally
case '*' in '*') ;; *) printf '%s\n' "FAIL: single-quoted * not literal" >&2; exit 1 ;; esac

# Unquoted special chars are still special
case "abc" in *) ;; nothing) printf '%s\n' "FAIL: unquoted * not special" >&2; exit 1 ;; esac

exit 0
