# SHALL-20-122-03-001
# "The time utility shall invoke the utility named by the utility operand with
#  arguments supplied as the argument operands and write a message to standard
#  error that lists timing statistics for the utility."
# Verify time invokes a utility and writes timing stats to stderr.

_err=$("${SHELL:-sh}" -c 'time -p true' 2>&1 >/dev/null)
if [ -z "$_err" ]; then
  printf '%s\n' "FAIL: time produced no stderr output" >&2
  exit 1
fi

# stderr must contain "real", "user", and "sys"
case "$_err" in
  *real*) ;;
  *) printf '%s\n' "FAIL: no 'real' in time output" >&2; exit 1 ;;
esac
case "$_err" in
  *user*) ;;
  *) printf '%s\n' "FAIL: no 'user' in time output" >&2; exit 1 ;;
esac
case "$_err" in
  *sys*) ;;
  *) printf '%s\n' "FAIL: no 'sys' in time output" >&2; exit 1 ;;
esac

exit 0
