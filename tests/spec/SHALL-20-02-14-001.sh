# SHALL-20-02-14-001
# "The following exit values shall be returned:"
# Test that alias returns 0 on success, >0 on error.

"$MEIKSH" -c 'alias mytest="echo hi"; exit $?'
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: alias did not return 0 on success" >&2
  exit 1
fi

"$MEIKSH" -c 'alias no_such_alias_xyz 2>/dev/null; exit $?'
if [ $? -eq 0 ]; then
  printf '%s\n' "FAIL: alias returned 0 for nonexistent alias" >&2
  exit 1
fi
exit 0
