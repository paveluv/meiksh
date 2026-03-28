# SHALL-20-133-05-001
# "The following operand shall be supported:"
# Verify unalias accepts the alias-name operand.

alias _test_ua1='true'
unalias _test_ua1 2>/dev/null
if alias _test_ua1 >/dev/null 2>&1; then
  printf '%s\n' "FAIL: unalias did not accept alias-name operand" >&2
  exit 1
fi

exit 0
