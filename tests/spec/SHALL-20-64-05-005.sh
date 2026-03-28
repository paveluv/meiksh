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

# Signal-based exit status: 128+2=130 -> INT
_name=$(kill -l 130 2>/dev/null)
case "$_name" in
  *INT*) ;;
  *)
    printf '%s\n' "FAIL: kill -l 130 gave '$_name', expected INT" >&2
    exit 1
    ;;
esac

# Signal-based exit status: 128+1=129 -> HUP
_name=$(kill -l 129 2>/dev/null)
case "$_name" in
  *HUP*) ;;
  *)
    printf '%s\n' "FAIL: kill -l 129 gave '$_name', expected HUP" >&2
    exit 1
    ;;
esac

exit 0
