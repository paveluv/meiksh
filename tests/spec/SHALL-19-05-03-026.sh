# SHALL-19-05-03-026
# "PPID ... Set by the shell to the decimal value of its parent process ID
#  during initialization ... In a subshell, PPID shall be set to the same value
#  as that of the parent of the current shell."

fail=0

# PPID should be numeric
case "$PPID" in
  *[!0-9]*) printf '%s\n' "FAIL: PPID not numeric: '$PPID'" >&2; fail=1 ;;
esac

# PPID in subshell should match
parent_ppid=$PPID
child_ppid=$(printf '%s' "$PPID")
[ "$parent_ppid" = "$child_ppid" ] || { printf '%s\n' "FAIL: subshell PPID differs: '$child_ppid' vs '$parent_ppid'" >&2; fail=1; }

# PPID in ( ) subshell
child_ppid=$( (printf '%s' "$PPID") )
[ "$parent_ppid" = "$child_ppid" ] || { printf '%s\n' "FAIL: () subshell PPID differs" >&2; fail=1; }

exit "$fail"
