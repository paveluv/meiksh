# SHALL-20-100-03-012
# "If exactly one var operand is yet to be processed and there was some
#  remaining unsplit input returned from the modified field splitting
#  algorithm, the variable named by the unprocessed var operand shall be
#  assigned the unsplit input."
# Verifies: last variable gets the unsplit remainder.

unset IFS
printf 'one two three four\n' | {
  read a b
  if [ "$a" != "one" ]; then
    printf '%s\n' "FAIL: a='$a' expected 'one'" >&2; exit 1
  fi
  if [ "$b" != "two three four" ]; then
    printf '%s\n' "FAIL: b='$b' expected 'two three four'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
