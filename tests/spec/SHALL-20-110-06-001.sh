# reviewed: GPT-5.4
# SHALL-20-110-06-001
# "The standard input shall be used only if one of the following is true:"
# Verifies: sh does not read stdin when -c or command_file is given.

SH="${MEIKSH:-${SHELL:-sh}}"

# With -c, stdin should not be consumed for commands
out=$(printf 'printf "%%s\\n" STDIN_CMD\n' | "$SH" -c 'printf "%s\n" from_c')
if [ "$out" != "from_c" ]; then
  printf '%s\n' "FAIL: -c should not read stdin: '$out'" >&2; exit 1
fi

# With command_file, stdin should not be consumed for commands
tmpf="$TMPDIR/stdin_test_$$.sh"
printf 'printf "%%s\\n" from_file\n' > "$tmpf"
out=$(printf 'printf "%%s\\n" STDIN_CMD\n' | "$SH" "$tmpf")
rm -f "$tmpf"
if [ "$out" != "from_file" ]; then
  printf '%s\n' "FAIL: command_file should not read stdin: '$out'" >&2; exit 1
fi

exit 0
