# SHALL-20-100-03-018
# "If end-of-file is detected before a terminating logical line delimiter is
#  encountered, the variables specified by the var operands shall be set as
#  described above and the exit status shall be 1."
# Verifies: EOF without newline sets vars and returns exit status 1.

# printf without trailing newline → EOF before delimiter
printf 'noterm' | {
  read var
  st=$?
  if [ "$var" != "noterm" ]; then
    printf '%s\n' "FAIL: var='$var' expected 'noterm'" >&2; exit 1
  fi
  if [ "$st" -ne 1 ]; then
    printf '%s\n' "FAIL: exit status=$st expected 1" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

# Multiple vars with EOF
printf 'one two' | {
  read a b
  st=$?
  if [ "$a" != "one" ] || [ "$b" != "two" ]; then
    printf '%s\n' "FAIL: a='$a' b='$b'" >&2; exit 1
  fi
  if [ "$st" -ne 1 ]; then
    printf '%s\n' "FAIL: exit status=$st expected 1" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
