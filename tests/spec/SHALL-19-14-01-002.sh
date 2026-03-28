# Test: SHALL-19-14-01-002
# Obligation: "In a pattern, or part of one, where a shell-quoting <backslash>
#   can be used, a <backslash> character shall escape the following character"
# Verifies: backslash escapes special pattern characters in shell-quoting context.

# \* matches literal *
case '*' in \*) ;; *) printf '%s\n' "FAIL: \\* did not match literal *" >&2; exit 1 ;; esac
case 'a' in \*) printf '%s\n' "FAIL: \\* matched 'a'" >&2; exit 1 ;; *) ;; esac

# \? matches literal ?
case '?' in \?) ;; *) printf '%s\n' "FAIL: \\? did not match literal ?" >&2; exit 1 ;; esac
case 'a' in \?) printf '%s\n' "FAIL: \\? matched 'a'" >&2; exit 1 ;; *) ;; esac

# \[ matches literal [
case '[' in \[) ;; *) printf '%s\n' "FAIL: \\[ did not match literal [" >&2; exit 1 ;; esac

# \\ matches literal backslash
case '\' in \\) ;; *) printf '%s\n' "FAIL: \\\\ did not match literal \\" >&2; exit 1 ;; esac

exit 0
