# SHALL-19-04-003
# "This recognition shall only occur when none of the characters is quoted and
#  when the word is used as:: The first word of a command"
# Verify reserved words are recognized only as the first word of a command.

fail=0

# 'if' as first word → reserved word (should parse correctly)
eval 'if true; then true; fi' || { printf '%s\n' "FAIL: 'if' not recognized as first word" >&2; fail=1; }

# 'if' as second word → NOT a reserved word (argument to echo)
result=$(printf '%s\n' if)
[ "$result" = "if" ] || { printf '%s\n' "FAIL: 'if' as argument not literal: '$result'" >&2; fail=1; }

exit "$fail"
