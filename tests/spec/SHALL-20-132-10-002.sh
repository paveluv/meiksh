# reviewed: GPT-5.4
# Also covers: SHALL-20-132-04-004
# SHALL-20-132-10-002
# "If -S is specified, the message shall be in the following format:
#  \"u=%s,g=%s,o=%s\\n\""
# Verify umask -S output format.

_old=$(umask)
umask 077
_out=$(umask -S)
umask "$_old"

case "$_out" in
  u=*,g=*,o=*) ;;
  *) printf '%s\n' "FAIL: format mismatch, got '$_out'" >&2; exit 1 ;;
esac

case "$_out" in
  *", "*|*" "*)
    printf '%s\n' "FAIL: unexpected spaces in umask -S output: '$_out'" >&2
    exit 1
    ;;
esac

exit 0
