# SHALL-19-05-03-008
# "IFS ... A string treated as a list of characters that is used for field
#  splitting ... If IFS is not set, it shall behave as normal for an unset
#  variable, except that field splitting ... shall be performed as if the value
#  of IFS is <space><tab><newline>. The shell shall set IFS to
#  <space><tab><newline> when it is invoked."

fail=0

# Default IFS splits on space/tab/newline
x="a b	c
d"
set -- $x
[ $# -eq 4 ] || { printf '%s\n' "FAIL: default IFS split count = $#, expected 4" >&2; fail=1; }

# Custom IFS
IFS=:
x="a:b:c"
set -- $x
[ $# -eq 3 ] || { printf '%s\n' "FAIL: IFS=: split count = $#, expected 3" >&2; fail=1; }
[ "$2" = "b" ] || { printf '%s\n' "FAIL: IFS=: field 2 = '$2'" >&2; fail=1; }

# Empty IFS — no splitting
IFS=
x="a b c"
set -- $x
[ $# -eq 1 ] || { printf '%s\n' "FAIL: empty IFS split count = $#, expected 1" >&2; fail=1; }

# Unset IFS — behaves as default
unset IFS
x="a b c"
set -- $x
[ $# -eq 3 ] || { printf '%s\n' "FAIL: unset IFS split count = $#, expected 3" >&2; fail=1; }

exit "$fail"
