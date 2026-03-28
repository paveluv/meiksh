# SHALL-19-05-02-001
# "Listed below are the special parameters and the values to which they shall
#  expand."
# Verify all special parameters are recognized and expand to something.

fail=0

# $@ — positional params
set -- a b c
result="$@"
[ -n "$result" ] || { printf '%s\n' "FAIL: \$@ empty with positional params set" >&2; fail=1; }

# $* — positional params as single string
result="$*"
[ -n "$result" ] || { printf '%s\n' "FAIL: \$* empty with positional params set" >&2; fail=1; }

# $# — count
[ "$#" = "3" ] || { printf '%s\n' "FAIL: \$# = '$#', expected 3" >&2; fail=1; }

# $? — last exit status
true
[ "$?" = "0" ] || { printf '%s\n' "FAIL: \$? after true = '$?'" >&2; fail=1; }

# $$ — PID (should be numeric)
case "$$" in
  *[!0-9]*) printf '%s\n' "FAIL: \$\$ not numeric: '$$'" >&2; fail=1 ;;
esac

# $0 — shell name (should be non-empty)
[ -n "$0" ] || { printf '%s\n' "FAIL: \$0 is empty" >&2; fail=1; }

exit "$fail"
