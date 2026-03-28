# SHALL-19-05-02-005
# "$? ... Expands to the shortest representation of the decimal exit status of
#  the pipeline ... The value of the special parameter '?' shall be set to 0
#  during initialization of the shell."

fail=0

# $? after successful command
true
[ "$?" = "0" ] || { printf '%s\n' "FAIL: \$? after true = '$?'" >&2; fail=1; }

# $? after failing command
false
[ "$?" = "1" ] || { printf '%s\n' "FAIL: \$? after false = '$?'" >&2; fail=1; }

# $? after exit 42 in subshell
(exit 42)
[ "$?" = "42" ] || { printf '%s\n' "FAIL: \$? after (exit 42) = '$?'" >&2; fail=1; }

# $? preserved in subshell
result=$("${MEIKSH:-sh}" -c 'printf "%s" "$?"')
[ "$result" = "0" ] || { printf '%s\n' "FAIL: \$? at shell init = '$result'" >&2; fail=1; }

# Command substitution exit status
x=$(exit 7)
[ "$?" = "7" ] || { printf '%s\n' "FAIL: \$? after cmd sub exit 7 = '$?'" >&2; fail=1; }

exit "$fail"
