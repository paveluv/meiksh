# SHALL-20-133-11-001
# "The standard error shall be used only for diagnostic messages."
# Verify unalias writes diagnostics to stderr (not stdout) on error.

_out=$(unalias _no_such_alias_ever_ 2>/dev/null)
if [ -n "$_out" ]; then
  printf '%s\n' "FAIL: unalias wrote non-diagnostic output to stdout" >&2
  exit 1
fi

_err=$(unalias _no_such_alias_ever_ 2>&1 >/dev/null)
if [ -z "$_err" ]; then
  printf '%s\n' "FAIL: unalias did not write diagnostic to stderr for unknown alias" >&2
  exit 1
fi

exit 0
