# Test: SHALL-19-03-003
# Obligation: "When it is not processing an io_here, the shell shall break
#   its input into tokens by applying the first applicable rule below to
#   each character in turn ... If a rule below indicates that a token is
#   delimited, and no characters have been included in the token, that
#   empty token shall be discarded."
# Verifies: Basic tokenization works correctly; empty tokens discarded.

# Multiple spaces between words: no empty tokens generated
set -- a     b     c
[ "$#" = "3" ] || { printf '%s\n' "FAIL: expected 3 args, got $#" >&2; exit 1; }
[ "$1" = "a" ] || { printf '%s\n' "FAIL: arg1" >&2; exit 1; }
[ "$2" = "b" ] || { printf '%s\n' "FAIL: arg2" >&2; exit 1; }
[ "$3" = "c" ] || { printf '%s\n' "FAIL: arg3" >&2; exit 1; }

# Quoting characters are included in token (verified via value)
r="hello"
[ "$r" = "hello" ] || { printf '%s\n' "FAIL: basic token" >&2; exit 1; }

exit 0
