# reviewed: GPT-5.4
# SHALL-20-110-05-011
# "A string that shall be interpreted by the shell as one or more commands
#  ... If the command_string operand is an empty string, sh shall exit with
#  a zero exit status."
# Verifies docs/posix/issue8/sh-utility.html#tag_20_110_05:
# empty command_string exits 0, and non-empty command_string is interpreted
# by the shell as one or more commands.

SH="${MEIKSH:-sh}"

# Empty string → exit 0
"$SH" -c '' >/dev/null 2>&1
st=$?
if [ "$st" -ne 0 ]; then
  printf '%s\n' "FAIL: empty command_string exit=$st expected 0" >&2; exit 1
fi

# Non-empty → executed
out=$("$SH" -c 'printf "%s\n" executed')
if [ "$out" != "executed" ]; then
  printf '%s\n' "FAIL: non-empty command_string: '$out'" >&2; exit 1
fi

# Multiple commands and shell interpretation → assignment and later expansion
out=$("$SH" -c 'VALUE=multi; printf "%s\n" first; printf "%s\n" "$VALUE"')
expected=$(printf '%s\n%s' first multi)
if [ "$out" != "$expected" ]; then
  printf '%s\n' "FAIL: multi-command command_string: '$out'" >&2; exit 1
fi

exit 0
