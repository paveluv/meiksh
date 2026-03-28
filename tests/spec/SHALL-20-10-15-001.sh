# SHALL-20-10-15-001
# "If job control is disabled, the bg utility shall exit with an error and no job
#  shall be placed in the background."

"$MEIKSH" +m -c 'bg 2>/dev/null; exit $?'
if [ $? -eq 0 ]; then
  printf '%s\n' "FAIL: bg returned 0 with job control disabled" >&2
  exit 1
fi
exit 0
