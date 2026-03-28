# SHALL-20-133-14-002
# "The following exit values shall be returned:: 0"
# Verify unalias returns exit status 0 on successful removal.

alias _test_ex0='true'
unalias _test_ex0
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: unalias exit status $_rc, expected 0" >&2
  exit 1
fi

exit 0
