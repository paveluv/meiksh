# SHALL-20-62-14-002
# "The following exit values shall be returned:: 0"
# Verify: jobs returns 0 on successful invocation with no arguments.

jobs
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: jobs exited $_rc, expected 0" >&2
  exit 1
fi

exit 0
