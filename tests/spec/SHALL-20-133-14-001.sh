# SHALL-20-133-14-001
# "The following exit values shall be returned:"
# Verify unalias returns 0 on success and >0 on error.

alias _test_exit_ua='true'
unalias _test_exit_ua
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: unalias returned $_rc on success, expected 0" >&2
  exit 1
fi

unalias _no_such_alias_ever_ 2>/dev/null
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: unalias returned 0 for nonexistent alias" >&2
  exit 1
fi

exit 0
