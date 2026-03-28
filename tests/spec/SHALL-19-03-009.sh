# SHALL-19-03-009
# "If the current character is not quoted and can be used as the first character
#  of a new operator, the current token (if any) shall be delimited. The current
#  character shall be used as the beginning of the next (operator) token."

fail=0

# Pipe delimits token: echo hello|cat should work (hello delimited by |)
result=$(eval 'printf hello|cat')
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: pipe delimit = '$result'" >&2; fail=1; }

# Semicolon delimits token
result=$(eval 'printf first;printf second')
[ "$result" = "firstsecond" ] || { printf '%s\n' "FAIL: semicolon delimit = '$result'" >&2; fail=1; }

# & in background (just check parsing works)
eval 'true& wait' 2>/dev/null
[ $? -eq 0 ] || { printf '%s\n' "FAIL: & parsing failed" >&2; fail=1; }

# Redirection operator delimits token
result=$(eval 'printf hello>/dev/null; printf ok')
[ "$result" = "ok" ] || { printf '%s\n' "FAIL: redirect delimit = '$result'" >&2; fail=1; }

exit "$fail"
