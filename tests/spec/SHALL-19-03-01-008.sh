# SHALL-19-03-01-008
# "An implementation may defer the effect of a change to an alias but the change
#  shall take effect no later than the completion of the currently executing
#  complete_command. Changes to aliases shall not take effect out of order."
# Verify alias change takes effect by next complete_command.

fail=0

# Define alias in one complete_command, use in the next
eval 'alias testcmd="printf aliased\n"'
result=$(eval 'testcmd')
[ "$result" = "aliased" ] || { printf '%s\n' "FAIL: alias not effective by next command: '$result'" >&2; fail=1; }

# Verify ordering: define A then B, B should see A's effect
eval 'alias ordA="printf A\n"; alias ordB="printf B\n"'
rA=$(eval 'ordA')
rB=$(eval 'ordB')
[ "$rA" = "A" ] && [ "$rB" = "B" ] || { printf '%s\n' "FAIL: alias ordering wrong: A='$rA' B='$rB'" >&2; fail=1; }

unalias testcmd ordA ordB 2>/dev/null

exit "$fail"
