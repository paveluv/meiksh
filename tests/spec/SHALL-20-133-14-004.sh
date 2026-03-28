# SHALL-20-133-14-004
# "The following exit values shall be returned:: >0"
# Verify unalias returns >0 when alias-name is not defined.

unalias _no_such_alias_ever_ 2>/dev/null
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: unalias returned 0 for undefined alias, expected >0" >&2
  exit 1
fi

exit 0
