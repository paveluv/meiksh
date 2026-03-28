# SHALL-20-22-11-001
# "The standard error shall be used only for diagnostic messages."

fail=0

# command -v for a found command should produce nothing on stderr
stderr_out=$(command -v ls 2>&1 >/dev/null)
if [ -n "$stderr_out" ]; then
  printf 'FAIL: command -v ls produced stderr: %s\n' "$stderr_out" >&2
  fail=1
fi

# command execution mode should produce nothing extra on stderr
stderr_out=$(command true 2>&1 >/dev/null)
if [ -n "$stderr_out" ]; then
  printf 'FAIL: command true produced stderr: %s\n' "$stderr_out" >&2
  fail=1
fi

exit "$fail"
