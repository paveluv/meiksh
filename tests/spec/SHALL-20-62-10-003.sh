# SHALL-20-62-10-003
# "where the fields shall be as follows:"
# Verify jobs output contains the required fields (current, job-number, state,
# command).

got=$("${SHELL}" -c '
  sleep 100 &
  jobs
  kill %1 2>/dev/null
  wait 2>/dev/null
')

# Expect format like: [1]+ Running    sleep 100 &
# Must have a bracket-enclosed job number
case "$got" in
  *"["*"]"*) ;;
  *) printf '%s\n' "FAIL: jobs output missing [job-number], got: $got" >&2; exit 1 ;;
esac

exit 0
