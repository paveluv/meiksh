# SHALL-09-03-05-011
# "A non-matching list expression begins with a <circumflex> ('^'), and the
#  matching behavior shall be the logical inverse of the corresponding matching
#  list expression ... The <circumflex> shall have this special meaning only
#  when it occurs first in the list, immediately following the <left-square-bracket>."
# Verify non-matching (negated) bracket expressions with ^ and !.

case "d" in
  [^abc]) ;;
  *) printf '%s\n' "FAIL: [^abc] did not match 'd'" >&2; exit 1 ;;
esac

case "a" in
  [^abc]) printf '%s\n' "FAIL: [^abc] matched 'a'" >&2; exit 1 ;;
  *) ;;
esac

# Shell also supports ! for negation
case "x" in
  [!abc]) ;;
  *) printf '%s\n' "FAIL: [!abc] did not match 'x'" >&2; exit 1 ;;
esac

case "b" in
  [!abc]) printf '%s\n' "FAIL: [!abc] matched 'b'" >&2; exit 1 ;;
  *) ;;
esac

# ^ not first has no special meaning — [a^b] matches a, ^, or b
case "^" in
  [a^b]) ;;
  *) printf '%s\n' "FAIL: [a^b] did not match '^'" >&2; exit 1 ;;
esac

exit 0
