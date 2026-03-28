# SHALL-20-133-05-003
# "The following operand shall be supported:: The name of an alias to be removed."
# Verify unalias removes the named alias from the shell environment.

alias _test_uc='printf hello'
unalias _test_uc
if alias _test_uc >/dev/null 2>&1; then
  printf '%s\n' "FAIL: alias _test_uc still defined after unalias" >&2
  exit 1
fi

exit 0
