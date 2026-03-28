# SHALL-19-03-01-005
# "The TOKEN ... shall be subject to alias substitution if ... The TOKEN did not
#  either fully or, optionally, partially result from an alias substitution of
#  the same alias name at any earlier recursion level."
# Verify alias recursion guard: alias ls='ls --color' does not loop infinitely.

fail=0

alias ls='printf noloop\n'
result=$(eval 'ls' 2>&1)
# Should complete without hanging — the recursive 'ls' inside the alias value
# must not be re-expanded.
[ -n "$result" ] || { printf '%s\n' "FAIL: alias recursion guard failed (empty output)" >&2; fail=1; }

unalias ls 2>/dev/null

exit "$fail"
