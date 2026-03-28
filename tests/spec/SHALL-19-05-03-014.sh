# SHALL-19-05-03-014
# "LC_COLLATE ... Determine the behavior of range expressions, equivalence
#  classes, and multi-character collating elements within pattern matching."
# Verify LC_COLLATE is recognized as a variable.

fail=0

result=$(LC_COLLATE=C "${MEIKSH:-sh}" -c 'printf "%s" "$LC_COLLATE"')
[ "$result" = "C" ] || { printf '%s\n' "FAIL: LC_COLLATE not imported: '$result'" >&2; fail=1; }

exit "$fail"
