# SHALL-07-03-01-009
# "In the POSIX locale, exactly <space>, <form-feed>, <newline>,
#  <carriage-return>, <tab>, and <vertical-tab> shall be included."
# Verify [[:space:]] matches the required whitespace characters.

LC_ALL=POSIX
export LC_ALL

fail=0

sp=' '
tab="$(printf '\t')"
nl="$(printf '\nX')"
nl="${nl%X}"

case "$sp" in
  [[:space:]]) ;;
  *) printf '%s\n' "FAIL: space not matched by [[:space:]]" >&2; fail=1 ;;
esac

case "$tab" in
  [[:space:]]) ;;
  *) printf '%s\n' "FAIL: tab not matched by [[:space:]]" >&2; fail=1 ;;
esac

# Letters and digits must not match
for c in a A 0; do
  case "$c" in
    [[:space:]]) printf '%s\n' "FAIL: '$c' matched by [[:space:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
