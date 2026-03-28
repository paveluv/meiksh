# reviewed: GPT-5.4
# Also covers: SHALL-20-64-05-001, SHALL-20-64-05-002
# SHALL-20-64-05-003
# "A decimal integer specifying a process or process group to be signaled.
#  The process or processes selected by positive, negative, and zero values
#  of the pid operand shall be as described for the kill() function."
# Verifies docs/posix/utilities/kill.html#tag_20_64_05:
# positive, zero, and negative pid operands are accepted in the required forms.

# Test 1: positive pid is accepted for an existing process
kill -s 0 "$$" 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -s 0 $$ returned $_rc, expected 0" >&2
  exit 1
fi

# Test 2: kill -s 0 with pid 0 targets current process group (existence test)
kill -s 0 0 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -s 0 0 returned $_rc, expected 0" >&2
  exit 1
fi

# Test 3: negative first pid operand should work when preceded by --
_pgid=$(ps -o pgid= -p $$ 2>/dev/null | tr -d ' ')
if [ -z "$_pgid" ]; then
  printf '%s\n' "FAIL: could not determine current process group id" >&2
  exit 1
fi

kill -s 0 -- "-$_pgid" 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -s 0 -- -$_pgid returned $_rc, expected 0" >&2
  exit 1
fi

exit 0
