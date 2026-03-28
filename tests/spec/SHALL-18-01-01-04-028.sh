# SHALL-18-01-01-04-028
# "When a file is to be appended, the file shall be opened in a manner
#  equivalent to using the O_APPEND flag, without the O_TRUNC flag, in the
#  open() function"
# Verify >> appends without truncating.

tmpf="$TMPDIR/shall_18_04_028_$$"
printf '%s\n' "line1" > "$tmpf"

"${SHELL}" -c 'printf "%s\n" "line2" >> "'"$tmpf"'"'
content=$(cat "$tmpf")
rm -f "$tmpf"

expected="line1
line2"
if [ "$content" != "$expected" ]; then
  printf '%s\n' "FAIL: append did not preserve original content" >&2
  printf '%s\n' "got: '$content'" >&2
  exit 1
fi

exit 0
