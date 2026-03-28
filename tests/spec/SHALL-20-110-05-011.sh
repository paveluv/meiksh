# SHALL-20-110-05-011
# "A string that shall be interpreted by the shell as one or more commands
#  ... If the command_string operand is an empty string, sh shall exit with
#  a zero exit status."
# Verifies: empty command_string exits 0; non-empty is executed.

SH="${MEIKSH:-sh}"

# Empty string → exit 0
"$SH" -c ''
st=$?
if [ "$st" -ne 0 ]; then
  printf '%s\n' "FAIL: empty command_string exit=$st expected 0" >&2; exit 1
fi

# Non-empty → executed
out=$("$SH" -c 'printf "%s\n" executed')
if [ "$out" != "executed" ]; then
  printf '%s\n' "FAIL: non-empty command_string: '$out'" >&2; exit 1
fi

exit 0
