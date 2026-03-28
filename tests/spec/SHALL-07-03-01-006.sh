# SHALL-07-03-01-006
# "In the POSIX locale, only characters in the classes upper and lower shall be
#  included."
# Verify [[:alpha:]] matches exactly A-Za-z in POSIX locale.

LC_ALL=POSIX
export LC_ALL

fail=0

for c in a b c d e f g h i j k l m n o p q r s t u v w x y z \
         A B C D E F G H I J K L M N O P Q R S T U V W X Y Z; do
  case "$c" in
    [[:alpha:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:alpha:]]" >&2; fail=1 ;;
  esac
done

for c in 0 1 2 3 4 5 6 7 8 9; do
  case "$c" in
    [[:alpha:]]) printf '%s\n' "FAIL: '$c' matched by [[:alpha:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
