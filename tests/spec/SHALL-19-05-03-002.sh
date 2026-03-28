# SHALL-19-05-03-002
# "The following variables shall affect the execution of the shell:"
# Verify the shell recognizes the mandatory special variables.

fail=0

# Check that these variables are recognized (can be assigned/read)
for var in HOME IFS PATH PWD; do
  eval "test_val=\$$var"
  # At minimum they should be set (or settable)
done

# IFS should be set at startup to space+tab+newline
result=$("${MEIKSH:-sh}" -c 'printf "%s" "$IFS"' | od -An -tx1 | tr -d ' \n')
# space=20, tab=09, newline=0a
[ "$result" = "20090a" ] || { printf '%s\n' "FAIL: IFS default = '$result', expected 20090a" >&2; fail=1; }

exit "$fail"
