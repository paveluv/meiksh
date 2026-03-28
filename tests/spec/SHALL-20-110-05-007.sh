# reviewed: GPT-5.4
# SHALL-20-110-05-007
# "The pathname of a file containing commands ... Special parameter 0 ...
#  shall be set to the value of command_file."
# Verifies: $0 is set to command_file pathname when running a script.

SH="${MEIKSH:-${SHELL:-sh}}"
tmpf="$TMPDIR/dollar0_test_$$.sh"
printf 'printf "%%s\\n" "$0"\n' > "$tmpf"
out=$("$SH" "$tmpf")
rm -f "$tmpf"
if [ "$out" != "$tmpf" ]; then
  printf '%s\n' "FAIL: \$0='$out' expected '$tmpf'" >&2; exit 1
fi

exit 0
