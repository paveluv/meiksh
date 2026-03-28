# SHALL-19-05-02-004
# "$# ... Expands to the shortest representation of the decimal number of
#  positional parameters. The command name (parameter 0) shall not be counted."

fail=0

set -- a b c
[ "$#" = "3" ] || { printf '%s\n' "FAIL: \$# = '$#', expected 3" >&2; fail=1; }

set --
[ "$#" = "0" ] || { printf '%s\n' "FAIL: \$# = '$#', expected 0" >&2; fail=1; }

set -- x
[ "$#" = "1" ] || { printf '%s\n' "FAIL: \$# = '$#', expected 1" >&2; fail=1; }

# Verify no leading zeros
set -- a b c d e f g h i j
[ "$#" = "10" ] || { printf '%s\n' "FAIL: \$# = '$#', expected 10" >&2; fail=1; }

exit "$fail"
