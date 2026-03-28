# SHALL-20-62-10-012
# "The character '+' identifies the job that would be used as a default for
#  the fg or bg utilities ... The character '-' identifies the job that would
#  become the default if the current default job were to exit ... At most one
#  job can be identified with '+' and at most one job can be identified with
#  '-'. If there is any suspended job, then the current job shall be a
#  suspended job."
# Verify + and - indicators with multiple background jobs.

got=$("${MEIKSH:-meiksh}" -c '
  sleep 100 &
  sleep 100 &
  out=$(jobs)
  kill %1 %2 2>/dev/null
  wait 2>/dev/null
  printf "%s\n" "$out"
')

plus_count=$(printf '%s\n' "$got" | grep -c '+')
minus_count=$(printf '%s\n' "$got" | grep -c '\-' || true)

if [ "$plus_count" -ne 1 ]; then
  printf '%s\n' "FAIL: expected exactly 1 '+' indicator, got $plus_count" >&2
  exit 1
fi

exit 0
