# reviewed: GPT-5.4
# SHALL-20-64-10-004
# "When both the -l option and exit_status operand are specified, the
#  symbolic name of the corresponding signal shall be written in the
#  following format:"
# Verify: kill -l <exit_status> outputs a single signal name followed
#  by newline.

# kill -l 9 -> exactly "KILL\n"
_raw=$(kill -l 9 2>/dev/null; printf x)
_expected=$(printf 'KILL\nx')
if [ "$_raw" != "$_expected" ]; then
  printf '%s\n' "FAIL: kill -l 9 output did not match exact format" >&2
  exit 1
fi

# Real signal-derived exit_status -> exactly one matching signal name and newline
(sh -c 'kill -1 $$' >/dev/null 2>&1) 2>/dev/null
_status=$?
_raw=$(kill -l "$_status" 2>/dev/null; printf x)
_expected=$(printf 'HUP\nx')
if [ "$_raw" != "$_expected" ]; then
  printf '%s\n' "FAIL: kill -l \$_status=$_status output did not match exact HUP format" >&2
  exit 1
fi

exit 0
