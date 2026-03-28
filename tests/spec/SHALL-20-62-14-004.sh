# SHALL-20-62-14-004
# "The following exit values shall be returned:: >0"
# Verify: jobs returns >0 when given an invalid job_id.

jobs %nonexistent_job_id_99999 2>/dev/null
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: jobs returned 0 for invalid job_id, expected >0" >&2
  exit 1
fi

exit 0
