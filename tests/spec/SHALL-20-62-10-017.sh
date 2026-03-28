# SHALL-20-62-10-017
# "For non-job-control background jobs (if supported), a field containing the
#  process ID associated with the job shall be inserted before the <state>
#  field."
# Verify jobs -l shows PID for non-job-control background jobs.

got=$("${SHELL}" +m -c '
  sleep 100 &
  bgpid=$!
  out=$(jobs -l)
  kill "$bgpid" 2>/dev/null
  wait 2>/dev/null
  printf "%s\n" "$out"
')

# Should contain a numeric PID
case "$got" in
  *[0-9][0-9][0-9]*) ;;
  *) printf '%s\n' "FAIL: jobs -l (no job control) should include PID, got: $got" >&2; exit 1 ;;
esac

exit 0
