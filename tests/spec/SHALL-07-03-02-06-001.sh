# SHALL-07-03-02-06-001
# "All characters not explicitly listed here shall be inserted in the character
#  collation order after the listed characters... The collation sequence shall
#  not include any multi-character collating elements."
# Verify POSIX locale collation uses code-point order for bracket ranges.

LC_ALL=POSIX
export LC_ALL

fail=0

# In POSIX locale, [a-z] must match only lowercase via code-point order
for c in a m z; do
  case "$c" in
    [a-z]) ;;
    *) printf '%s\n' "FAIL: '$c' not in [a-z]" >&2; fail=1 ;;
  esac
done

# Uppercase must NOT be in [a-z] in POSIX locale
for c in A M Z; do
  case "$c" in
    [a-z]) printf '%s\n' "FAIL: '$c' in [a-z] (POSIX locale)" >&2; fail=1 ;;
    *) ;;
  esac
done

# [A-Z] must match only uppercase
for c in A M Z; do
  case "$c" in
    [A-Z]) ;;
    *) printf '%s\n' "FAIL: '$c' not in [A-Z]" >&2; fail=1 ;;
  esac
done

for c in a m z; do
  case "$c" in
    [A-Z]) printf '%s\n' "FAIL: '$c' in [A-Z] (POSIX locale)" >&2; fail=1 ;;
    *) ;;
  esac
done

exit "$fail"
