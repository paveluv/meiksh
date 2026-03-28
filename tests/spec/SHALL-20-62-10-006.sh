# SHALL-20-62-10-006
# "where the fields shall be as follows:: <job-number>"
# Verify jobs output includes a numeric job number in brackets.

got=$("${MEIKSH:-meiksh}" -c '
  sleep 100 &
  jobs
  kill %1 2>/dev/null
  wait 2>/dev/null
')

# Must contain [N] where N is a number
case "$got" in
  *"[1]"*|*"[2]"*|*"[3]"*) ;;
  *) printf '%s\n' "FAIL: jobs output missing [N] job number, got: $got" >&2; exit 1 ;;
esac

exit 0
