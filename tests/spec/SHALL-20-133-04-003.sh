# SHALL-20-133-04-003
# "The following option shall be supported: -a"
# Verify unalias -a is accepted.

alias _test_a1='true'
alias _test_a2='true'
unalias -a
_found=$(alias 2>/dev/null)
# After -a, there should be no aliases (or at minimum, ours are gone)
case "$_found" in
  *_test_a1*|*_test_a2*)
    printf '%s\n' "FAIL: aliases remain after unalias -a" >&2; exit 1 ;;
esac

exit 0
