# SHALL-20-110-05-009
# "A string assigned to special parameter 0 when executing the commands in
#  command_string. If command_name is not specified, special parameter 0
#  shall be set to the value of the first argument passed to sh from its
#  parent"
# Verifies: $0 is set from command_name with -c.

SH="${MEIKSH:-sh}"

# With command_name
out=$("$SH" -c 'printf "%s\n" "$0"' myname)
if [ "$out" != "myname" ]; then
  printf '%s\n' "FAIL: \$0 with command_name: '$out'" >&2; exit 1
fi

exit 0
