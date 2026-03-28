# SHALL-20-100-03-001
# "The read utility shall read a single logical line from standard input into one
#  or more shell variables."

result=$("$MEIKSH" -c '
  printf "hello world\n" | read var1 var2
  printf "%s %s\n" "$var1" "$var2"
' 2>/dev/null)
# Pipelines run in subshells in most shells, use heredoc instead
result=$("$MEIKSH" -c '
  read var1 var2 <<EOF
hello world
EOF
  printf "%s|%s\n" "$var1" "$var2"
')
if [ "$result" != "hello|world" ]; then
  printf '%s\n' "FAIL: read did not split into variables, got: $result" >&2
  exit 1
fi
exit 0
