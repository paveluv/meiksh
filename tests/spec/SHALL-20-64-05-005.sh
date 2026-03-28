# reviewed: GPT-5.4
# Also covers: SHALL-20-64-05-001, SHALL-20-64-05-004
# SHALL-20-64-05-005
# "A decimal integer specifying a signal number or the exit status of a
#  process terminated by a signal."
# Verify: kill -l accepts both raw signal numbers and signal-based exit
#  statuses.

# Raw signal number: 15 -> TERM
_name=$(kill -l 15 2>/dev/null)
case "$_name" in
  *TERM*) ;;
  *)
    printf '%s\n' "FAIL: kill -l 15 gave '$_name', expected TERM" >&2
    exit 1
    ;;
esac

# Signal-based exit status: derive a real $? value from a signal-terminated child
(sh -c 'kill -1 $$' >/dev/null 2>&1) 2>/dev/null
_status=$?
_name=$(kill -l "$_status" 2>/dev/null)
case "$_name" in
  *HUP*) ;;
  *)
    printf '%s\n' "FAIL: kill -l \$_status=$_status gave '$_name', expected HUP" >&2
    exit 1
    ;;
esac

exit 0
