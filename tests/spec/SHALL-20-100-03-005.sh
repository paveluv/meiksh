# SHALL-20-100-03-005
# "If IFS is unset, or is set to any non-empty value, then a modified
#  version of the field splitting algorithm specified in 2.6.5 Field
#  Splitting shall be applied"
# Verifies: read performs field splitting when IFS is unset and when
#   IFS is set to a non-empty value.

# IFS unset → default splitting on spaces/tabs/newlines
unset IFS
printf 'one two three\n' | {
  read a b c
  if [ "$a" != "one" ] || [ "$b" != "two" ] || [ "$c" != "three" ]; then
    printf '%s\n' "FAIL: IFS unset split: a='$a' b='$b' c='$c'" >&2
    exit 1
  fi
}
[ $? -ne 0 ] && exit 1

# IFS set to non-empty → splits on that character
IFS=:
printf 'x:y:z\n' | {
  read a b c
  if [ "$a" != "x" ] || [ "$b" != "y" ] || [ "$c" != "z" ]; then
    printf '%s\n' "FAIL: IFS=: split: a='$a' b='$b' c='$c'" >&2
    exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
