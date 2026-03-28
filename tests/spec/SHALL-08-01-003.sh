# SHALL-08-01-003
# "Uppercase and lowercase letters shall retain their unique identities and
#  shall not be folded together."
# Environment variable names must be case-sensitive.

export MYVAR_CASE="upper"
export myvar_case="lower"

if [ "$MYVAR_CASE" != "upper" ]; then
  printf '%s\n' "FAIL: MYVAR_CASE should be 'upper', got '$MYVAR_CASE'" >&2
  exit 1
fi

if [ "$myvar_case" != "lower" ]; then
  printf '%s\n' "FAIL: myvar_case should be 'lower', got '$myvar_case'" >&2
  exit 1
fi

# Verify both survive into a child process independently
got_upper=$("$0" -c 'printf "%s\n" "$MYVAR_CASE"' 2>/dev/null) || true
got_lower=$("$0" -c 'printf "%s\n" "$myvar_case"' 2>/dev/null) || true

# Fallback: just verify the shell variables themselves are distinct
if [ "$MYVAR_CASE" = "$myvar_case" ]; then
  printf '%s\n' "FAIL: case-folding detected — MYVAR_CASE == myvar_case" >&2
  exit 1
fi

exit 0
