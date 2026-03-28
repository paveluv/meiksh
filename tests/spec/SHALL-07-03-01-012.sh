# SHALL-07-03-01-012
# "In the POSIX locale, all characters in classes alpha, digit, and punct shall
#  be included; no characters in class cntrl shall be included."
# Verify [[:graph:]] matches alpha, digit, punct but not space or cntrl.

LC_ALL=POSIX
export LC_ALL

fail=0

# Graph must match letters, digits, punctuation
for c in a z A Z 0 9 '!' '#' '.' '_'; do
  case "$c" in
    [[:graph:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:graph:]]" >&2; fail=1 ;;
  esac
done

# Graph must NOT match space
case ' ' in
  [[:graph:]]) printf '%s\n' "FAIL: space matched by [[:graph:]]" >&2; fail=1 ;;
  *) ;;
esac

exit "$fail"
