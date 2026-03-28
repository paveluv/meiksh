# SHALL-09-03-05-009
# "A bracket expression is either a matching list expression or a non-matching
#  list expression ... The <right-square-bracket> (']') shall lose its special
#  meaning and represent itself in a bracket expression if it occurs first in
#  the list ... special characters '.', '*', '[', and '\\' shall lose their
#  special meaning within the bracket expression"
# Verify ] first in bracket, and special chars lose meaning inside brackets.

# ] first in list matches literal ]
case "]" in
  []ab]) ;;
  *) printf '%s\n' "FAIL: ] first in list did not match ']'" >&2; exit 1 ;;
esac

# * inside bracket matches literal *
case "*" in
  [*ab]) ;;
  *) printf '%s\n' "FAIL: [*ab] did not match '*'" >&2; exit 1 ;;
esac

# ? inside bracket matches literal ?
case "?" in
  [?ab]) ;;
  *) printf '%s\n' "FAIL: [?ab] did not match '?'" >&2; exit 1 ;;
esac

# [ inside bracket matches literal [
case "[" in
  [[]) ;;
  *) printf '%s\n' "FAIL: [[] did not match '['" >&2; exit 1 ;;
esac

# * inside bracket should NOT glob-expand
case "x" in
  [*]) printf '%s\n' "FAIL: [*] matched 'x' (should only match '*')" >&2; exit 1 ;;
esac

exit 0
