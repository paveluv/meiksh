# SHALL-07-03-01-015
# "In the POSIX locale, only the <space> and <tab> shall be included."
# Verify [[:blank:]] matches space and tab, not other characters.

LC_ALL=POSIX
export LC_ALL

fail=0

tab="$(printf '\t')"

case ' ' in
  [[:blank:]]) ;;
  *) printf '%s\n' "FAIL: space not matched by [[:blank:]]" >&2; fail=1 ;;
esac

case "$tab" in
  [[:blank:]]) ;;
  *) printf '%s\n' "FAIL: tab not matched by [[:blank:]]" >&2; fail=1 ;;
esac

for c in a A 0 '!'; do
  case "$c" in
    [[:blank:]]) printf '%s\n' "FAIL: '$c' matched by [[:blank:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
