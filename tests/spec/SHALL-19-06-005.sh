# SHALL-19-06-005
# "Quote removal, if performed, shall always be performed last."
# Verify quote characters are removed from the final result.

fail=0

# Double quotes removed
result=$(printf '%s' "hello")
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: double-quote removal: '$result'" >&2; fail=1; }

# Single quotes removed
result=$(printf '%s' 'hello')
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: single-quote removal: '$result'" >&2; fail=1; }

# Backslash removed from escaped char
result=$(printf '%s' hell\o)
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: backslash removal: '$result'" >&2; fail=1; }

# Mixed quoting all removed
result=$(printf '%s' "hel"'lo')
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: mixed quote removal: '$result'" >&2; fail=1; }

exit "$fail"
