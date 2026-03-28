# SHALL-19-04-005
# "This recognition shall only occur when none of the characters is quoted and
#  when the word is used as:: The third word in a case command (only in is valid
#  in this case)"
# Verify 'in' is recognized as reserved word in third position of case command.

fail=0

eval 'case foo in foo) printf matched;; esac'
result=$(eval 'case foo in foo) printf matched;; esac')
[ "$result" = "matched" ] || { printf '%s\n' "FAIL: 'in' not recognized in case: '$result'" >&2; fail=1; }

exit "$fail"
