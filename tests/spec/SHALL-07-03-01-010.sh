# SHALL-07-03-01-010
# "In the POSIX locale, no characters in classes alpha or print shall be
#  included."
# Verify [[:cntrl:]] does not match alpha or print characters.

LC_ALL=POSIX
export LC_ALL

fail=0

for c in a z A Z 0 9 '!' ' ' '~'; do
  case "$c" in
    [[:cntrl:]]) printf '%s\n' "FAIL: '$c' matched by [[:cntrl:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
