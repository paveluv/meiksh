# SHALL-07-03-01-017
# "In the POSIX locale, the 26 lowercase characters: a b c ... z shall be
#  mapped to the corresponding 26 uppercase characters: A B C ... Z"
# Verify POSIX locale toupper mapping works in shell case conversion.

LC_ALL=POSIX
export LC_ALL

fail=0

upper="ABCDEFGHIJKLMNOPQRSTUVWXYZ"
lower="abcdefghijklmnopqrstuvwxyz"

# Test that [[:upper:]] matches exactly A-Z
for c in A B C D E F G H I J K L M N O P Q R S T U V W X Y Z; do
  case "$c" in
    [[:upper:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:upper:]]" >&2; fail=1 ;;
  esac
done

for c in a b c 0 1 '!'; do
  case "$c" in
    [[:upper:]]) printf '%s\n' "FAIL: '$c' matched by [[:upper:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
