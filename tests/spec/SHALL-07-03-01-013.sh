# SHALL-07-03-01-013
# "In the POSIX locale, all characters in class graph shall be included; no
#  characters in class cntrl shall be included."
# Verify [[:print:]] matches graph chars and space.

LC_ALL=POSIX
export LC_ALL

fail=0

# Print must match letters, digits, punctuation, and space
for c in a z A Z 0 9 '!' '#' '.' '_' ' '; do
  case "$c" in
    [[:print:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:print:]]" >&2; fail=1 ;;
  esac
done

exit "$fail"
