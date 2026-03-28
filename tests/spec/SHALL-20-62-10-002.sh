# SHALL-20-62-10-002
# "Otherwise, if the -l option is not specified, the output shall be a series
#  of lines of the form:"
# Verify default jobs output format: "[%d] %c %s %s\n"
# (job-number, current indicator, state, command)

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  jobs 2>/dev/null
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
')

# Output should contain bracketed job number
case "$_out" in
  *\[*\]*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs output should contain [job-number], got: $_out" >&2
    exit 1
    ;;
esac

# Should contain a state (Running)
case "$_out" in
  *[Rr]unning*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs output should contain 'Running', got: $_out" >&2
    exit 1
    ;;
esac

# Should contain the command
case "$_out" in
  *sleep*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs output should contain command 'sleep', got: $_out" >&2
    exit 1
    ;;
esac

exit 0
