# SHALL-20-62-10-004
# "where the fields shall be as follows:: <current>"
# Verify jobs output includes the current-job indicator field (+, -, or space).

got=$("${SHELL}" -c '
  sleep 100 &
  jobs
  kill %1 2>/dev/null
  wait 2>/dev/null
')

# The most recent job should have a + indicator
case "$got" in
  *+*) ;;
  *) printf '%s\n' "FAIL: jobs output missing '+' current indicator, got: $got" >&2; exit 1 ;;
esac

exit 0
