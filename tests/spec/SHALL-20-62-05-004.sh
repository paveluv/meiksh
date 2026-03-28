# SHALL-20-62-05-004
# "If no job_id is given, the status information for all jobs shall be
#  displayed."
# Verify jobs with no arguments lists all jobs.

got=$("${SHELL}" -c '
  sleep 100 &
  sleep 100 &
  out=$(jobs)
  kill %1 %2 2>/dev/null
  wait 2>/dev/null
  printf "%s\n" "$out"
')

# Count lines — should have at least 2 job entries
lines=$(printf '%s\n' "$got" | grep -c '.')
if [ "$lines" -lt 2 ]; then
  printf '%s\n' "FAIL: jobs should list 2 background jobs, got $lines lines" >&2
  exit 1
fi

exit 0
