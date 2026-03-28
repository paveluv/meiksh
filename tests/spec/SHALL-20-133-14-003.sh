# SHALL-20-133-14-003
# "The following exit values shall be returned:: Successful completion."
# Verify unalias exit 0 means all aliases were removed.

alias _test_sc1='true'
alias _test_sc2='false'
unalias _test_sc1 _test_sc2
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: unalias exit $_rc, expected 0 for successful removal" >&2
  exit 1
fi

exit 0
