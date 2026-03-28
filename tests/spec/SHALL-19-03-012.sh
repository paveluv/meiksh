# SHALL-19-03-012
# "If the current character is a '#', it and all subsequent characters up to,
#  but excluding, the next <newline> shall be discarded as a comment."
# Verify # starts a comment that is discarded.

fail=0

# Comment at start of line
result=$(eval 'printf before
# this is a comment
printf after')
[ "$result" = "beforeafter" ] || { printf '%s\n' "FAIL: comment not discarded: '$result'" >&2; fail=1; }

# Comment after command
result=$(eval 'printf visible # not visible')
[ "$result" = "visible" ] || { printf '%s\n' "FAIL: inline comment not discarded: '$result'" >&2; fail=1; }

# Hash in middle of word is NOT a comment
result=$(printf '%s\n' 'he#llo')
[ "$result" = "he#llo" ] || { printf '%s\n' "FAIL: mid-word hash treated as comment: '$result'" >&2; fail=1; }

exit "$fail"
