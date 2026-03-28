# SHALL-20-62-03-002
# "When jobs reports the termination status of a job, the shell shall remove
#  the job from the background jobs list"
# Verify completed jobs are removed from the list after being reported.

_out=$(sh -c '
  true &
  wait $!
  jobs
  jobs
')

# After reporting termination, second jobs call should produce no output
# (or at least not repeat the terminated job)
_count=$(printf '%s\n' "$_out" | grep -c '[Dd]one' 2>/dev/null || true)
if [ "$_count" -gt 1 ]; then
  printf '%s\n' "FAIL: terminated job should be removed after report, shown $_count times" >&2
  exit 1
fi

exit 0
