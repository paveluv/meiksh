# SHALL-19-29-04-003
# "Write to standard output a list of commands associated with each condition
#  operand. The shell shall format the output, including the proper use of quoting,
#  so that it is suitable for reinput to the shell..."

result=$("$MEIKSH" -c '
  trap "echo hi" INT
  trap "echo bye" TERM
  out=$(trap -p INT)
  case "$out" in
    *"echo hi"*INT*) exit 0 ;;
  esac
  exit 1
')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: trap -p with condition did not output correct trap" >&2
  exit 1
fi
exit 0
