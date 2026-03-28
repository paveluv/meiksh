# SHALL-19-05-03-018
# "LC_MESSAGES ... Determine the language in which messages should be written."
# Verify LC_MESSAGES is recognized as a variable.

fail=0

result=$(LC_MESSAGES=C "${MEIKSH:-sh}" -c 'printf "%s" "$LC_MESSAGES"')
[ "$result" = "C" ] || { printf '%s\n' "FAIL: LC_MESSAGES not imported: '$result'" >&2; fail=1; }

exit "$fail"
