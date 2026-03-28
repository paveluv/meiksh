# SHALL-07-03-01-011
# "In the POSIX locale, neither the <space> nor any characters in classes alpha,
#  digit, or cntrl shall be included."
# Verify [[:punct:]] does not match space, alpha, digit chars; does match punctuation.

LC_ALL=POSIX
export LC_ALL

fail=0

# Punct must not match space, letters, digits
for c in ' ' a z A Z 0 9; do
  case "$c" in
    [[:punct:]]) printf '%s\n' "FAIL: '$c' matched by [[:punct:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

# Punct must match punctuation characters
for c in '!' '#' '%' '.' ',' ':' ';' '?' '_' '-'; do
  case "$c" in
    [[:punct:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:punct:]]" >&2; fail=1 ;;
  esac
done

exit "$fail"
