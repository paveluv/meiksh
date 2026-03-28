# SHALL-19-06-002
# "Tilde expansion, parameter expansion, command substitution, and arithmetic
#  expansion shall be performed, beginning to end."
# Verify all four step-1 expansions are performed left-to-right.

fail=0

HOME=/tmp
x=world

# All four in one word: ~, $x, $(cmd), $((arith))
result=$(eval 'printf "%s\n" "~=$HOME x=$x cmd=$(printf sub) arith=$((2+3))"')
expected="~=/tmp x=world cmd=sub arith=5"
[ "$result" = "$expected" ] || { printf '%s\n' "FAIL: step-1 expansions = '$result'" >&2; fail=1; }

# Left-to-right: param expansion result feeds into nothing further
y='$(printf NOPE)'
result=$(printf '%s' "$y")
[ "$result" = '$(printf NOPE)' ] || { printf '%s\n' "FAIL: param expansion result re-expanded" >&2; fail=1; }

exit "$fail"
