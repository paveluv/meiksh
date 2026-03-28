# reviewed: GPT-5.4
# Also covers: SHALL-20-64-04-002
# SHALL-20-64-04-003
# "The following options shall be supported:: -l"
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# kill -l is supported both without an operand and with a decimal operand.

_out=$(kill -l 2>/dev/null)
if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: kill -l produced no output" >&2
  exit 1
fi

# Output must contain at least HUP, INT, KILL, TERM
for _sig in HUP INT KILL TERM; do
  case "$_out" in
    *"$_sig"*) ;;
    *)
      printf '%s\n' "FAIL: kill -l output missing $_sig" >&2
      exit 1
      ;;
  esac
done

_one=$(kill -l 15 2>/dev/null)
if [ "$_one" != "TERM" ]; then
  printf '%s\n' "FAIL: kill -l 15 returned '$_one', expected 'TERM'" >&2
  exit 1
fi

exit 0
