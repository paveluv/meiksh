# SHALL-20-62-10-016
# "For job-control background jobs, a field containing the process group ID
#  shall be inserted before the <state> field."
# Verify jobs -l shows PGID for job-control background jobs.

got=$("${MEIKSH:-meiksh}" -mc '
  sleep 100 &
  bgpid=$!
  out=$(jobs -l)
  kill "$bgpid" 2>/dev/null
  wait 2>/dev/null
  printf "%s\n" "$out"
')

# Should contain a numeric ID (PGID)
case "$got" in
  *[0-9][0-9][0-9]*) ;;
  *) printf '%s\n' "FAIL: jobs -l should include PGID, got: $got" >&2; exit 1 ;;
esac

exit 0
