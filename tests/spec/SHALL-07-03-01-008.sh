# SHALL-07-03-01-008
# "Only the characters specified for the alpha and digit keywords shall be
#  specified."
# Verify [[:alnum:]] matches exactly A-Za-z0-9 in POSIX locale.

LC_ALL=POSIX
export LC_ALL

fail=0

for c in a b c d e f g h i j k l m n o p q r s t u v w x y z \
         A B C D E F G H I J K L M N O P Q R S T U V W X Y Z \
         0 1 2 3 4 5 6 7 8 9; do
  case "$c" in
    [[:alnum:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:alnum:]]" >&2; fail=1 ;;
  esac
done

for c in '!' '@' '#' ' '; do
  case "$c" in
    [[:alnum:]]) printf '%s\n' "FAIL: '$c' matched by [[:alnum:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
