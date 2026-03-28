# SHALL-07-03-01-018
# "In the POSIX locale, the 26 uppercase characters: A B C ... Z shall be
#  mapped to the corresponding 26 lowercase characters: a b c ... z"
# Verify POSIX locale tolower mapping via [[:lower:]] classification.

LC_ALL=POSIX
export LC_ALL

fail=0

for c in a b c d e f g h i j k l m n o p q r s t u v w x y z; do
  case "$c" in
    [[:lower:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:lower:]]" >&2; fail=1 ;;
  esac
done

for c in A B C 0 1 '!'; do
  case "$c" in
    [[:lower:]]) printf '%s\n' "FAIL: '$c' matched by [[:lower:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
