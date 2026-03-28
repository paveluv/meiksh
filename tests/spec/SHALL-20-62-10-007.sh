# SHALL-20-62-10-007
# "A number that can be used to identify the job to the wait, fg, bg, and kill
#  utilities. Using these utilities, the job can be identified by prefixing the
#  job number with '%'."
# Verify job number from jobs output works with %n syntax.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  _jobline=$(jobs 2>/dev/null)
  kill %1 2>/dev/null
  _rc=$?
  wait "$_pid" 2>/dev/null
  exit $_rc
')

if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: kill %%1 should work using job number from jobs" >&2
  exit 1
fi

exit 0
