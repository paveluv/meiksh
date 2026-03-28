# SHALL-19-05-03-020
# "LINENO ... Set by the shell to a decimal number representing the current
#  sequential line number (numbered starting with 1) within a script or function
#  before it executes each command."
# Verify LINENO tracks line numbers in a script.

fail=0

# Run a small script and check LINENO values
script="$TMPDIR/lineno_test_$$.sh"
cat > "$script" << 'SCRIPT'
l1=$LINENO
l2=$LINENO
l3=$LINENO
printf '%s %s %s\n' "$l1" "$l2" "$l3"
SCRIPT

result=$("${MEIKSH:-sh}" "$script")
rm -f "$script"

# Lines should be 1, 2, 3 (or at least ascending sequential)
set -- $result
[ "$1" = "1" ] || { printf '%s\n' "FAIL: LINENO line 1 = '$1'" >&2; fail=1; }
[ "$2" = "2" ] || { printf '%s\n' "FAIL: LINENO line 2 = '$2'" >&2; fail=1; }
[ "$3" = "3" ] || { printf '%s\n' "FAIL: LINENO line 3 = '$3'" >&2; fail=1; }

exit "$fail"
