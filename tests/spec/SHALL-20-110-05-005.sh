# reviewed: GPT-5.4
# SHALL-20-110-05-005
# "The positional parameters ($1, $2, and so on) shall be set to arguments,
#  if any."
# Verifies: positional parameters are set from script arguments.

SH="${MEIKSH:-${SHELL:-sh}}"
tmpf="$TMPDIR/pos_test_$$.sh"
printf 'printf "%%s %%s %%s\\n" "$1" "$2" "$3"\n' > "$tmpf"
out=$("$SH" "$tmpf" aa bb cc)
rm -f "$tmpf"
if [ "$out" != "aa bb cc" ]; then
  printf '%s\n' "FAIL: positional params: '$out'" >&2; exit 1
fi

exit 0
