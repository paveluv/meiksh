# SHALL-07-03-01-005
# "In the POSIX locale, only: a b c d e f g h i j k l m n o p q r s t u v w x y z
#  shall be included."
# Verify [[:lower:]] matches exactly a-z in POSIX locale.

LC_ALL=POSIX
export LC_ALL

fail=0

# Every lowercase letter must match
for c in a b c d e f g h i j k l m n o p q r s t u v w x y z; do
  case "$c" in
    [[:lower:]]) ;;
    *) printf '%s\n' "FAIL: '$c' not matched by [[:lower:]]" >&2; fail=1 ;;
  esac
done

# Uppercase letters must NOT match
for c in A B C D E F G H I J K L M N O P Q R S T U V W X Y Z; do
  case "$c" in
    [[:lower:]]) printf '%s\n' "FAIL: '$c' matched by [[:lower:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

# Digits must NOT match
for c in 0 1 2 3 4 5 6 7 8 9; do
  case "$c" in
    [[:lower:]]) printf '%s\n' "FAIL: '$c' matched by [[:lower:]]" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
