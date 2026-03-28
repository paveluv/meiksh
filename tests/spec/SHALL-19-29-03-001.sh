# SHALL-19-29-03-001
# "If the -p option is not specified and the first operand is an unsigned decimal
#  integer, the shell shall treat all operands as conditions, and shall reset each
#  condition to the default value. Otherwise, if the -p option is not specified and
#  there are operands, the first operand shall be treated as an action and the
#  remaining as conditions."

# Test 1: first operand is unsigned decimal integer -> all operands are conditions (reset)
result=$("${SHELL:-sh}" -c '
  trap "echo caught" INT TERM
  trap 2 15
  out=$(trap)
  if [ -z "$out" ]; then
    exit 0
  else
    exit 1
  fi
')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: numeric first operand did not reset signals" >&2
  exit 1
fi

# Test 2: first operand is NOT an integer -> treated as action
result=$("${SHELL:-sh}" -c '
  trap "echo hello" INT
  out=$(trap -p INT)
  case "$out" in
    *"echo hello"*) exit 0 ;;
    *) exit 1 ;;
  esac
')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: string first operand not treated as action" >&2
  exit 1
fi
exit 0
