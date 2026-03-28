# SHALL-20-100-03-007
# "The loop over the contents of that initial field shall cease when either
#  the input is empty or n output fields have been generated, where n is one
#  less than the number of var operands passed to the read utility. Any
#  remaining input ... shall be returned to the read utility 'unsplit'; that
#  is, unmodified except that any leading or trailing IFS white space ...
#  shall be removed."
# Verifies: last var gets unsplit remainder with IFS whitespace trimmed.

unset IFS
printf '  one  two  three  four  \n' | {
  read a b c
  if [ "$a" != "one" ]; then
    printf '%s\n' "FAIL: a='$a' expected 'one'" >&2; exit 1
  fi
  if [ "$b" != "two" ]; then
    printf '%s\n' "FAIL: b='$b' expected 'two'" >&2; exit 1
  fi
  if [ "$c" != "three  four" ]; then
    printf '%s\n' "FAIL: c='$c' expected 'three  four'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
