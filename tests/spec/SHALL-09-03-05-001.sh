# SHALL-09-03-05-001
# "A bracket expression (an expression enclosed in square brackets, '[]') is
#  an RE that shall match a specific set of single characters"
# Verify bracket expressions match a single character from the set.

case "b" in
  [abc]) ;;
  *) printf '%s\n' "FAIL: [abc] did not match 'b'" >&2; exit 1 ;;
esac

case "x" in
  [abc]) printf '%s\n' "FAIL: [abc] matched 'x'" >&2; exit 1 ;;
esac

# ] first in list matches literal ]
case "]" in
  []abc]) ;;
  *) printf '%s\n' "FAIL: []abc] did not match ']'" >&2; exit 1 ;;
esac

exit 0
