# SHALL-09-03-05-015
# "In the POSIX locale, a range expression represents the set of collating
#  elements that fall between two elements in the collation sequence, inclusive.
#  A range expression shall be expressed as the starting point and the ending
#  point separated by a <hyphen-minus> ... The <hyphen-minus> character shall
#  be treated as itself if it occurs first or last in the list"
# Verify range expressions and literal hyphen placement in bracket expressions.

LC_ALL=POSIX
export LC_ALL

fail=0

# Basic range: [a-c] matches a, b, c
case "b" in
  [a-c]) ;;
  *) printf '%s\n' "FAIL: [a-c] did not match 'b'" >&2; fail=1 ;;
esac

case "d" in
  [a-c]) printf '%s\n' "FAIL: [a-c] matched 'd'" >&2; fail=1 ;;
  *) ;;
esac

# Hyphen first is literal: [-ac] matches -, a, c
case "-" in
  [-ac]) ;;
  *) printf '%s\n' "FAIL: [-ac] did not match '-'" >&2; fail=1 ;;
esac

# Hyphen last is literal: [ac-] matches a, c, -
case "-" in
  [ac-]) ;;
  *) printf '%s\n' "FAIL: [ac-] did not match '-'" >&2; fail=1 ;;
esac

# Digit range
case "5" in
  [0-9]) ;;
  *) printf '%s\n' "FAIL: [0-9] did not match '5'" >&2; fail=1 ;;
esac

# Upper range
case "M" in
  [A-Z]) ;;
  *) printf '%s\n' "FAIL: [A-Z] did not match 'M'" >&2; fail=1 ;;
esac

exit "$fail"
