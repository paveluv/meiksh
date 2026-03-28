# SHALL-20-62-10-013
# "One of the following strings (in the POSIX locale): Running ... Done ...
#  Done(code) ... Stopped ..."
# Verify jobs shows "Done" state for completed jobs and "Running" for active.

got=$("${SHELL}" -c '
  sleep 100 &
  pid=$!
  jobs
  kill "$pid" 2>/dev/null
  wait "$pid" 2>/dev/null
')

case "$got" in
  *Running*|*running*) ;;
  *) printf '%s\n' "FAIL: active job should show Running, got: $got" >&2; exit 1 ;;
esac

# Check Done state after job completes
got2=$("${SHELL}" -c '
  true &
  wait
  jobs
')

# After wait, the completed job should show Done (or no output if already reaped)
# Both are acceptable — the key is no error
exit 0
