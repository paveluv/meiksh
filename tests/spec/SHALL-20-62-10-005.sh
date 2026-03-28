# SHALL-20-62-10-005
# "The character '+' identifies the job that would be used as a default for the
#  fg or bg utilities... The character '-' identifies the job that would become
#  the default if the current default job were to exit..."
# Verify current (+) and previous (-) job markers in jobs output.

_out=$(sh -c '
  sleep 60 &
  sleep 61 &
  jobs 2>/dev/null
  kill %1 %2 2>/dev/null
  wait 2>/dev/null
')

# Should have a '+' marker on one job
case "$_out" in
  *+*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs should show + for current job, got: $_out" >&2
    exit 1
    ;;
esac

# Should have a '-' marker on another job (when >=2 jobs)
case "$_out" in
  *-*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs should show - for previous job, got: $_out" >&2
    exit 1
    ;;
esac

exit 0
