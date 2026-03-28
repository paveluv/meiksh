# SHALL-20-62-10-008
# "where the fields shall be as follows:: <state>"
# Verify jobs output includes a state field (Running, Done, Stopped, etc.).

got=$("${MEIKSH:-meiksh}" -c '
  sleep 100 &
  jobs
  kill %1 2>/dev/null
  wait 2>/dev/null
')

case "$got" in
  *Running*|*running*) ;;
  *) printf '%s\n' "FAIL: jobs output missing state field, got: $got" >&2; exit 1 ;;
esac

exit 0
