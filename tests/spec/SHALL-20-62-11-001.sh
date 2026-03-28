# SHALL-20-62-11-001
# "The standard error shall be used only for diagnostic messages."
# Verify: jobs produces no stderr output under normal operation.

_err=$(jobs 2>&1 1>/dev/null)
if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: jobs wrote to stderr during normal operation: $_err" >&2
  exit 1
fi

exit 0
