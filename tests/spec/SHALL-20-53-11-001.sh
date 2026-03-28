# SHALL-20-53-11-001
# "Whenever an error is detected and the first character in the optstring operand
#  is not a <colon> (':'), a diagnostic message shall be written to standard error"
# Verify verbose mode writes diagnostic to stderr on error.

# Test 1: unknown option in verbose mode produces stderr
OPTIND=1
_err=$(getopts "a" opt -z 2>&1 1>/dev/null)
if [ -z "$_err" ]; then
  printf '%s\n' "FAIL(1): unknown option should produce stderr diagnostic" >&2
  exit 1
fi

# Test 2: missing argument in verbose mode produces stderr
OPTIND=1
_err=$(getopts "b:" opt -b 2>&1 1>/dev/null)
if [ -z "$_err" ]; then
  printf '%s\n' "FAIL(2): missing argument should produce stderr diagnostic" >&2
  exit 1
fi

# Test 3: silent mode (leading colon) should NOT produce stderr
OPTIND=1
_err=$(getopts ":a" opt -z 2>&1 1>/dev/null)
if [ -n "$_err" ]; then
  printf '%s\n' "FAIL(3): silent mode should not produce stderr, got '$_err'" >&2
  exit 1
fi

exit 0
