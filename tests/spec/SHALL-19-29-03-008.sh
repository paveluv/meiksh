# SHALL-19-29-03-008
# "The trap command with no operands shall write to standard output a list of
#  commands associated with each of a set of conditions..."

result=$("$MEIKSH" -c '
  trap "echo hi" INT
  trap "" TERM
  out=$(trap)
  case "$out" in
    *INT*) ;;
    *) exit 1 ;;
  esac
  case "$out" in
    *TERM*) ;;
    *) exit 1 ;;
  esac
  exit 0
')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: trap with no operands did not list set traps" >&2
  exit 1
fi
exit 0
