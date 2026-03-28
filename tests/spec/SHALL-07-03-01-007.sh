# SHALL-07-03-01-007
# "In all locales, only: 0 1 2 3 4 5 6 7 8 9 shall be included."
# Verify [[:digit:]] matches exactly 0-9 in POSIX locale.

LC_ALL=POSIX
export LC_ALL

fail=0

for c in 0 1 2 3 4 5 6 7 8 9; do
  case "$c" in
    [[:digit:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:digit:]]" >&2; fail=1 ;;
  esac
done

for c in a b c A B C; do
  case "$c" in
    [[:digit:]]) printf '%s\n' "FAIL: '$c' matched by [[:digit:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
