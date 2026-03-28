# SHALL-20-100-03-015
# "If there are still one or more unprocessed var operands, each of the
#  variables names by those operands shall be assigned an empty string."
# Verifies: extra variables are set to empty when fewer fields than vars.

unset IFS
printf 'only\n' | {
  read a b c
  if [ "$a" != "only" ]; then
    printf '%s\n' "FAIL: a='$a' expected 'only'" >&2; exit 1
  fi
  if [ "$b" != "" ]; then
    printf '%s\n' "FAIL: b='$b' expected empty" >&2; exit 1
  fi
  if [ "$c" != "" ]; then
    printf '%s\n' "FAIL: c='$c' expected empty" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
