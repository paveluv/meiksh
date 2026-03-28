# SHALL-19-05-02-002
# "$@ ... Expands to the positional parameters, starting from one, initially
#  producing one field for each positional parameter that is set ... If there are
#  no positional parameters, the expansion of '@' shall generate zero fields,
#  even when '@' is within double-quotes."

fail=0

# "$@" produces separate fields
set -- a b c
count=0
for x in "$@"; do count=$((count+1)); done
[ "$count" = "3" ] || { printf '%s\n' "FAIL: \"\$@\" field count = $count, expected 3" >&2; fail=1; }

# Fields preserve values
set -- "hello world" "foo bar"
first=
for x in "$@"; do first="$x"; break; done
[ "$first" = "hello world" ] || { printf '%s\n' "FAIL: \"\$@\" first field = '$first'" >&2; fail=1; }

# Zero positional params → zero fields
set --
count=0
for x in "$@"; do count=$((count+1)); done
[ "$count" = "0" ] || { printf '%s\n' "FAIL: empty \"\$@\" produced $count fields" >&2; fail=1; }

exit "$fail"
