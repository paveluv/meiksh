# SHALL-20-147-11-001
# "The standard error shall be used only for diagnostic messages."

fail=0

# wait with no operands and no background jobs: no stdout, no stderr
out=$( (wait) 2>"$TMPDIR/wait_stderr_none" )
stderr_content=$(cat "$TMPDIR/wait_stderr_none")
if [ -n "$out" ]; then
  printf 'FAIL: wait produced unexpected stdout: %s\n' "$out" >&2
  fail=1
fi
if [ -n "$stderr_content" ]; then
  printf 'FAIL: wait produced unexpected stderr: %s\n' "$stderr_content" >&2
  fail=1
fi

# wait for a known PID: no stdout output
(exit 0) &
pid=$!
out=$(wait "$pid" 2>/dev/null)
if [ -n "$out" ]; then
  printf 'FAIL: wait $pid produced stdout: %s\n' "$out" >&2
  fail=1
fi

exit "$fail"
