# SHALL-19-05-02-003
# "$* ... When the expansion occurs in a context where field splitting will not
#  be performed, the initial fields shall be joined to form a single field with
#  the value of each parameter separated by the first character of the IFS
#  variable."

fail=0

set -- a b c

# Default IFS — "$*" joins with space
result="$*"
[ "$result" = "a b c" ] || { printf '%s\n' "FAIL: \"\$*\" default IFS = '$result'" >&2; fail=1; }

# Custom IFS — "$*" joins with first char
IFS=,
result="$*"
[ "$result" = "a,b,c" ] || { printf '%s\n' "FAIL: \"\$*\" IFS=, = '$result'" >&2; fail=1; }

# Empty IFS — "$*" joins with no separator
IFS=
result="$*"
[ "$result" = "abc" ] || { printf '%s\n' "FAIL: \"\$*\" IFS='' = '$result'" >&2; fail=1; }

# Unset IFS — "$*" joins with space
unset IFS
result="$*"
[ "$result" = "a b c" ] || { printf '%s\n' "FAIL: \"\$*\" unset IFS = '$result'" >&2; fail=1; }

exit "$fail"
