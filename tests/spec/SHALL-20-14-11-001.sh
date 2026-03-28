# SHALL-20-14-11-001
# "The standard error shall be used only for diagnostic messages."
# Verify cd stderr is only used for diagnostics.

_out=$(cd /nonexistent_dir_$$ 2>/dev/null)
if [ -n "$_out" ]; then
  printf '%s\n' "FAIL: cd error wrote to stdout" >&2
  exit 1
fi

_err=$(cd /nonexistent_dir_$$ 2>&1 >/dev/null)
if [ -z "$_err" ]; then
  printf '%s\n' "FAIL: cd error did not write diagnostic to stderr" >&2
  exit 1
fi

exit 0
