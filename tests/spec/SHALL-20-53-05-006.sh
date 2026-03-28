# SHALL-20-53-05-006
# "If a character is followed by a <colon>, the option shall be expected to
#  have an argument ... getopts shall interpret the characters following an
#  option character requiring arguments as an argument whether or not this is
#  done ... The characters <question-mark> and <colon> shall not be used as
#  option characters by an application."
# Verify getopts handles option-argument in same token and as separate token.

# Separate argument form: -b value
got=$("${SHELL}" -c '
  OPTIND=1
  getopts "ab:" opt -b hello
  printf "%s:%s\n" "$opt" "$OPTARG"
')
if [ "$got" != "b:hello" ]; then
  printf '%s\n' "FAIL: getopts -b hello gave '$got', expected 'b:hello'" >&2
  exit 1
fi

# Combined form: -bhello
got2=$("${SHELL}" -c '
  OPTIND=1
  getopts "ab:" opt -bhello
  printf "%s:%s\n" "$opt" "$OPTARG"
')
if [ "$got2" != "b:hello" ]; then
  printf '%s\n' "FAIL: getopts -bhello gave '$got2', expected 'b:hello'" >&2
  exit 1
fi

exit 0
