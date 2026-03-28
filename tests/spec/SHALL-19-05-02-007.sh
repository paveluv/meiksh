# SHALL-19-05-02-007
# "$$ ... Expands to the shortest representation of the decimal process ID of
#  the invoked shell. In a subshell, '$' shall expand to the same value as that
#  of the current shell."

fail=0

# $$ should be numeric
case "$$" in
  *[!0-9]*) printf '%s\n' "FAIL: \$\$ not numeric: '$$'" >&2; fail=1 ;;
esac

# $$ in subshell should match parent
parent_pid=$$
child_pid=$(printf '%s' "$$")
[ "$parent_pid" = "$child_pid" ] || { printf '%s\n' "FAIL: subshell \$\$ = '$child_pid', parent = '$parent_pid'" >&2; fail=1; }

# $$ in ( ) subshell
child_pid=$(eval '( printf "%s" "$$" )')
[ "$parent_pid" = "$child_pid" ] || { printf '%s\n' "FAIL: () subshell \$\$ = '$child_pid'" >&2; fail=1; }

exit "$fail"
