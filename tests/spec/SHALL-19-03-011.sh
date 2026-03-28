# SHALL-19-03-011
# "If the previous character was part of a word, the current character shall be
#  appended to that word."
# Verify ordinary characters continue a word token.

fail=0

# Letters, digits, hyphens, dots, slashes all continue the same word
result=$(printf '%s\n' abc-123.txt/path)
[ "$result" = "abc-123.txt/path" ] || { printf '%s\n' "FAIL: word continuation = '$result'" >&2; fail=1; }

# Underscore and equals in a word
result=$(printf '%s\n' key_name=value)
[ "$result" = "key_name=value" ] || { printf '%s\n' "FAIL: underscore/equals word = '$result'" >&2; fail=1; }

exit "$fail"
