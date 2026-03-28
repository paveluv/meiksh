# SHALL-20-62-10-010
# "where the fields shall be as follows:: <command>"
# Verify jobs output includes the command text.

got=$("${MEIKSH:-meiksh}" -c '
  sleep 100 &
  jobs
  kill %1 2>/dev/null
  wait 2>/dev/null
')

case "$got" in
  *sleep*) ;;
  *) printf '%s\n' "FAIL: jobs output missing command field, got: $got" >&2; exit 1 ;;
esac

exit 0
