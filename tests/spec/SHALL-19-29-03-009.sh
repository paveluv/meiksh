# SHALL-19-29-03-009
# "The shell shall format the output, including the proper use of quoting, so
#  that it is suitable for reinput to the shell as commands that achieve the same
#  trapping results..."

result=$("$MEIKSH" -c '
  trap "echo hello world" INT
  saved=$(trap -p INT)
  trap - INT
  eval "$saved"
  check=$(trap -p INT)
  case "$check" in
    *"echo hello world"*INT*) exit 0 ;;
    *) exit 1 ;;
  esac
')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: trap output not suitable for reinput" >&2
  exit 1
fi
exit 0
