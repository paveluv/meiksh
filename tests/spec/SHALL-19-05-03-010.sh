# SHALL-19-05-03-010
# "LANG ... Provide a default value for the internationalization variables that
#  are unset or null."
# Verify LANG provides default for LC_* when unset.

fail=0

# LANG should be recognized as a variable
result=$(LANG=C "${MEIKSH:-sh}" -c 'printf "%s" "$LANG"')
[ "$result" = "C" ] || { printf '%s\n' "FAIL: LANG not imported: '$result'" >&2; fail=1; }

exit "$fail"
