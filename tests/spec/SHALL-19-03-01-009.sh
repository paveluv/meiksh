# SHALL-19-03-01-009
# "... alias definitions shall not be inherited by separate invocations of the
#  shell or by the utility execution environments invoked by the shell."
# Verify aliases are not exported to child shells.

fail=0

alias secret_alias='printf "%s\n" leaked'

# Child shell should not have the alias
result=$("${MEIKSH:-sh}" -c 'secret_alias' 2>&1)
rc=$?
[ $rc -ne 0 ] || { printf '%s\n' "FAIL: alias inherited by child shell: '$result'" >&2; fail=1; }

unalias secret_alias 2>/dev/null

exit "$fail"
