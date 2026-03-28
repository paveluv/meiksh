# SHALL-07-03-01-01-001
# "Implementations may add additional characters to the cntrl and punct
#  classifications but shall not make any other additions."
# Verify POSIX locale character classes have no unexpected extras:
# [[:alpha:]] must be exactly A-Za-z, [[:digit:]] exactly 0-9.

LC_ALL=POSIX
export LC_ALL

fail=0

# Verify digits are exactly 0-9 by checking that common non-digit chars fail
for c in a A '!' ' '; do
  case "$c" in
    [[:digit:]]) printf '%s\n' "FAIL: '$c' matched by [[:digit:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

# Verify alpha is exactly A-Za-z by checking digits don't match
for c in 0 1 9; do
  case "$c" in
    [[:alpha:]]) printf '%s\n' "FAIL: '$c' matched by [[:alpha:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

# Verify space is exactly the 6 whitespace chars (spot check: letters must not match)
for c in a A 0 '!'; do
  case "$c" in
    [[:space:]]) printf '%s\n' "FAIL: '$c' matched by [[:space:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
