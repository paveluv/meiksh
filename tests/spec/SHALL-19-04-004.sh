# SHALL-19-04-004
# "This recognition shall only occur when none of the characters is quoted and
#  when the word is used as:: The first word following one of the reserved words
#  other than case, for, or in"
# Verify reserved word recognized after another reserved word (not case/for/in).

fail=0

# 'then' after 'if' is a reserved word
eval 'if true; then true; fi' || { printf '%s\n' "FAIL: then not recognized after if" >&2; fail=1; }

# Nested if: 'if' recognized after 'then'
eval 'if true; then if true; then true; fi; fi' || { printf '%s\n' "FAIL: nested if after then" >&2; fail=1; }

# Word after 'case' is NOT in reserved-word position
# 'case do in ...' — 'do' after 'case' is the case expression, not a keyword
eval 'case do in do) true;; esac' || { printf '%s\n' "FAIL: word after case treated as reserved" >&2; fail=1; }

exit "$fail"
