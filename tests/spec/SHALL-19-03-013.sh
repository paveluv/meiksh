# SHALL-19-03-013
# "... once a complete_command has been recognized by the grammar, the
#  complete_command shall be executed before the next complete_command is
#  tokenized and parsed."
# Verify that side effects of one command are visible to the next.

fail=0

# Variable assignment in one complete_command visible in next
result=$(eval 'x=hello
printf "%s\n" "$x"')
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: variable not visible in next command: '$result'" >&2; fail=1; }

# Function defined in one command, used in next
result=$(eval 'f() { printf funcok; }
f')
[ "$result" = "funcok" ] || { printf '%s\n' "FAIL: function not visible in next command: '$result'" >&2; fail=1; }

exit "$fail"
