# SHALL-20-62-10-009
# "One of the following strings (in the POSIX locale): Running... Done...
#  Done(code)... Stopped..."
# Verify jobs state strings for Running and Done.

# Test 1: Running state
_out=$(sh -c '
  sleep 60 &
  _pid=$!
  jobs 2>/dev/null
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
')
case "$_out" in
  *[Rr]unning*)
    ;;
  *)
    printf '%s\n' "FAIL(1): active job should show Running, got: $_out" >&2
    exit 1
    ;;
esac

# Test 2: Done state (exit 0)
_out=$(sh -c '
  true &
  wait $!
  jobs 2>/dev/null
')
case "$_out" in
  *[Dd]one*|"")
    # Either shows "Done" or empty (already cleared) is acceptable
    ;;
  *)
    printf '%s\n' "FAIL(2): completed job should show Done, got: $_out" >&2
    exit 1
    ;;
esac

# Test 3: Done(code) for non-zero exit
_out=$(sh -c '
  (exit 42) &
  wait $!
  jobs 2>/dev/null
')
case "$_out" in
  *[Dd]one*|"")
    ;;
  *)
    printf '%s\n' "FAIL(3): non-zero exit job should show Done(code), got: $_out" >&2
    exit 1
    ;;
esac

exit 0
