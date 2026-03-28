# SHALL-20-62-05-003
# "Specifies the jobs for which the status is to be displayed. If no job_id
#  is given, the status information for all jobs shall be displayed."
# Verify no job_id shows all jobs; specific job_id filters to that job.

_out=$(sh -c '
  sleep 61 &
  sleep 62 &
  _all=$(jobs 2>/dev/null)
  _one=$(jobs %1 2>/dev/null)
  kill %1 %2 2>/dev/null
  wait 2>/dev/null
  printf "ALL:%s\nONE:%s\n" "$_all" "$_one"
')

_all=$(printf '%s\n' "$_out" | grep '^ALL:' | sed 's/^ALL://')
_one=$(printf '%s\n' "$_out" | grep '^ONE:' | sed 's/^ONE://')

# All jobs output should contain both sleep commands
_all_count=$(printf '%s\n' "$_all" | grep -c 'sleep' 2>/dev/null || true)
if [ "$_all_count" -lt 2 ]; then
  printf '%s\n' "FAIL: jobs with no job_id should show all jobs (expected 2, got $_all_count)" >&2
  exit 1
fi

# Specific job_id should show only 1
_one_count=$(printf '%s\n' "$_one" | grep -c 'sleep' 2>/dev/null || true)
if [ "$_one_count" -ne 1 ]; then
  printf '%s\n' "FAIL: jobs %%1 should show 1 job, got $_one_count" >&2
  exit 1
fi

exit 0
