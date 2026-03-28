# SHALL-20-133-04-004
# "Remove all alias definitions from the current shell execution environment."
# Verify unalias -a removes all aliases.

alias _test_rmall1='echo a'
alias _test_rmall2='echo b'
alias _test_rmall3='echo c'
unalias -a

_count=0
for _name in _test_rmall1 _test_rmall2 _test_rmall3; do
  if alias "$_name" >/dev/null 2>&1; then
    _count=$(( _count + 1 ))
  fi
done

if [ "$_count" != "0" ]; then
  printf '%s\n' "FAIL: $_count aliases remain after unalias -a" >&2
  exit 1
fi

exit 0
