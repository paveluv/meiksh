# reviewed: GPT-5.4
# SHALL-20-110-04-011
# "If there are no operands and the -c option is not specified, the -s
#  option shall be assumed."
# Verifies the requirement in docs/posix/issue8/sh-utility.html#tag_20_110_04
# and its stdin consequence in docs/posix/issue8/sh-utility.html#tag_20_110_06:
# sh with no operands and no -c reads commands from standard input.

SH="${MEIKSH:-sh}"

out=$(printf 'printf "%%s\\n" implicit_s\n' | "$SH")
if [ "$out" != "implicit_s" ]; then
  printf '%s\n' "FAIL: implicit -s: '$out'" >&2; exit 1
fi

out=$(printf 'printf "%%s\\n" implicit_s_with_option\n' | "$SH" -u)
if [ "$out" != "implicit_s_with_option" ]; then
  printf '%s\n' "FAIL: implicit -s with option: '$out'" >&2; exit 1
fi

exit 0
