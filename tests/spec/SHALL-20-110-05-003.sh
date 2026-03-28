# reviewed: GPT-5.4
# SHALL-20-110-05-003
# "A single <hyphen-minus> shall be treated as the first operand and then
#  ignored."
# Verifies: sh - is accepted and the - operand is ignored.

SH="${MEIKSH:-${SHELL:-sh}}"

out=$(printf 'printf "%%s\\n" ok\n' | "$SH" -)
if [ "$out" != "ok" ]; then
  printf '%s\n' "FAIL: 'sh -' operand not ignored: '$out'" >&2; exit 1
fi

exit 0
