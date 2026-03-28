# SHALL-19-05-03-012
# "LC_ALL ... The value of this variable overrides the LC_* variables and LANG."
# Verify LC_ALL is recognized and overrides other locale vars.

fail=0

result=$(LC_ALL=C "${MEIKSH:-sh}" -c 'printf "%s" "$LC_ALL"')
[ "$result" = "C" ] || { printf '%s\n' "FAIL: LC_ALL not imported: '$result'" >&2; fail=1; }

exit "$fail"
