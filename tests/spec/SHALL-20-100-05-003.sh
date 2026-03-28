# SHALL-20-100-05-003
# "The name of an existing or nonexisting shell variable."
# Verifies: read can assign to both existing and new variables.

# Assign to nonexisting variable
unset NEWVAR 2>/dev/null
printf 'hello\n' | {
  read NEWVAR
  if [ "$NEWVAR" != "hello" ]; then
    printf '%s\n' "FAIL: nonexisting var: NEWVAR='$NEWVAR'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

# Assign to existing variable (overwrites)
EXISTVAR=old
printf 'new\n' | {
  EXISTVAR=old
  read EXISTVAR
  if [ "$EXISTVAR" != "new" ]; then
    printf '%s\n' "FAIL: existing var: EXISTVAR='$EXISTVAR'" >&2; exit 1
  fi
}
[ $? -ne 0 ] && exit 1

exit 0
