# SHALL-20-100-03-004
# "The terminating logical line delimiter (if any) shall be removed from the input.
#  Then, if the shell variable IFS is set, and its value is an empty string, the
#  resulting data shall be assigned to the variable named by the first var operand,
#  and the variables named by other var operands (if any) shall be set to the empty
#  string."

result=$("$MEIKSH" -c '
  IFS=""
  read var1 var2 <<EOF
hello world
EOF
  printf "%s|%s\n" "$var1" "$var2"
')
if [ "$result" != "hello world|" ]; then
  printf '%s\n' "FAIL: IFS=empty did not assign whole line to first var, got: $result" >&2
  exit 1
fi
exit 0
