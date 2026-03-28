# SHALL-19-05-03-001
# "Variables shall be initialized from the environment ... Shell variables shall
#  be initialized only from environment variables that have valid names. If a
#  variable is initialized from the environment, it shall be marked for export
#  immediately ... The shell shall set IFS to <space><tab><newline> when it is
#  invoked."

fail=0

# Verify env var with valid name is imported
result=$(MY_TEST_VAR=imported "${MEIKSH:-sh}" -c 'printf "%s" "$MY_TEST_VAR"')
[ "$result" = "imported" ] || { printf '%s\n' "FAIL: env var not imported: '$result'" >&2; fail=1; }

# Verify imported var is marked for export
result=$(MY_TEST_VAR=imported "${MEIKSH:-sh}" -c 'export | grep MY_TEST_VAR >/dev/null && printf yes')
[ "$result" = "yes" ] || { printf '%s\n' "FAIL: imported var not marked for export" >&2; fail=1; }

exit "$fail"
