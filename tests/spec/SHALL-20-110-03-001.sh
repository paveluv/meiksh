# reviewed: GPT-5.4
# SHALL-20-110-03-001
# "The sh utility is a command language interpreter that shall execute
#  commands read from a command line string, the standard input, or a
#  specified file."
# Verifies: sh executes commands from -c string, stdin, and a file.

SH="${MEIKSH:-${SHELL:-sh}}"

# From -c string
out=$("$SH" -c 'printf "%s\n" hello')
if [ "$out" != "hello" ]; then
  printf '%s\n' "FAIL: -c string: '$out'" >&2; exit 1
fi

# From a file
tmpf="$TMPDIR/sh_test_$$.sh"
printf 'printf "%%s\\n" fromfile\n' > "$tmpf"
out=$("$SH" "$tmpf")
rm -f "$tmpf"
if [ "$out" != "fromfile" ]; then
  printf '%s\n' "FAIL: from file: '$out'" >&2; exit 1
fi

# From stdin
out=$(printf 'printf "%%s\\n" fromstdin\n' | "$SH")
if [ "$out" != "fromstdin" ]; then
  printf '%s\n' "FAIL: from stdin: '$out'" >&2; exit 1
fi

exit 0
