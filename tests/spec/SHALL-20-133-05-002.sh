# SHALL-20-133-05-002
# "The following operand shall be supported:: alias-name"
# Verify unalias accepts one or more alias-name operands.

alias _test_ub1='true'
alias _test_ub2='false'
unalias _test_ub1 _test_ub2 2>/dev/null

_fail=0
if alias _test_ub1 >/dev/null 2>&1; then
  printf '%s\n' "FAIL: unalias did not remove _test_ub1" >&2
  _fail=1
fi
if alias _test_ub2 >/dev/null 2>&1; then
  printf '%s\n' "FAIL: unalias did not remove _test_ub2" >&2
  _fail=1
fi

exit "$_fail"
