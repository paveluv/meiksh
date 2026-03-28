# SHALL-19-05-03-022
# "NLSPATH ... Determine the location of message catalogs for the processing of
#  LC_MESSAGES."
# Verify NLSPATH is recognized as a variable.

fail=0

result=$(NLSPATH=/tmp "${MEIKSH:-sh}" -c 'printf "%s" "$NLSPATH"')
[ "$result" = "/tmp" ] || { printf '%s\n' "FAIL: NLSPATH not imported: '$result'" >&2; fail=1; }

exit "$fail"
