# SHALL-07-03-01-014
# "In all locales, only: 0 1 2 3 4 5 6 7 8 9 A B C D E F a b c d e f
#  shall be included."
# Verify [[:xdigit:]] matches exactly 0-9A-Fa-f in POSIX locale.

LC_ALL=POSIX
export LC_ALL

fail=0

for c in 0 1 2 3 4 5 6 7 8 9 A B C D E F a b c d e f; do
  case "$c" in
    [[:xdigit:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:xdigit:]]" >&2; fail=1 ;;
  esac
done

for c in g G z Z '!' ' '; do
  case "$c" in
    [[:xdigit:]]) printf '%s\n' "FAIL: '$c' matched by [[:xdigit:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
