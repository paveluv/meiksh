# reviewed: GPT-5.4
# SHALL-20-110-04-009
# "Read commands from the standard input."
# Verifies: -s reads commands from stdin.

SH="${MEIKSH:-${SHELL:-sh}}"

out=$(printf 'printf "%%s\\n" fromstdin\n' | "$SH" -s)
if [ "$out" != "fromstdin" ]; then
  printf '%s\n' "FAIL: -s from stdin: '$out'" >&2; exit 1
fi

exit 0
