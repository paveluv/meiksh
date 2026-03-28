# reviewed: GPT-5.4
# SHALL-20-64-10-002
# "When the -l option is specified, the symbolic name of each signal shall
#  be written in the following format:"
# Verifies docs/posix/utilities/kill.html#tag_20_64_10:
# kill -l writes signal names in POSIX signal-name form.

_out=$(kill -l 2>/dev/null)
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -l returned $_rc, expected 0" >&2
  exit 1
fi

if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: kill -l produced no output" >&2
  exit 1
fi

case "$_out" in
  *SIG*)
    printf '%s\n' "FAIL: kill -l output used SIG-prefixed names: '$_out'" >&2
    exit 1
    ;;
esac

# Must contain standard signal names
for _sig in HUP INT QUIT ABRT KILL ALRM TERM; do
  case "$_out" in
    *"$_sig"*) ;;
    *)
      printf '%s\n' "FAIL: kill -l missing signal $_sig" >&2
      exit 1
      ;;
  esac
done

exit 0
