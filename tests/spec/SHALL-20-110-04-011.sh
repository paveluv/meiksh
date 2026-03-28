# SHALL-20-110-04-011
# "If there are no operands and the -c option is not specified, the -s
#  option shall be assumed."
# Verifies: sh with no operands and no -c reads from stdin.

SH="${MEIKSH:-sh}"

out=$(printf 'printf "%%s\\n" implicit_s\n' | "$SH")
if [ "$out" != "implicit_s" ]; then
  printf '%s\n' "FAIL: implicit -s: '$out'" >&2; exit 1
fi

exit 0
