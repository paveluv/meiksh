# SHALL-19-04-006
# "This recognition shall only occur when none of the characters is quoted and
#  when the word is used as:: The third word in a for command (only in and do
#  are valid in this case)"
# Verify 'in' and 'do' recognized in third position of for command.

fail=0

# for NAME in WORDS ... do ... done
result=$(eval 'for i in a b; do printf "%s" "$i"; done')
[ "$result" = "ab" ] || { printf '%s\n' "FAIL: for/in form: '$result'" >&2; fail=1; }

# for NAME do ... done (iterates over positional params)
result=$(eval 'set -- x y; for i do printf "%s" "$i"; done')
[ "$result" = "xy" ] || { printf '%s\n' "FAIL: for/do form: '$result'" >&2; fail=1; }

exit "$fail"
