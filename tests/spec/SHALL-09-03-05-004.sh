# SHALL-09-03-05-004
# "A non-matching list expression begins with a <circumflex> ('^'), and the
#  matching behavior shall be the logical inverse of the corresponding
#  matching list expression"
# Verify [^abc] and [!abc] match characters NOT in the set.

case "d" in
  [^abc]) ;;
  *) printf '%s\n' "FAIL: [^abc] did not match 'd'" >&2; exit 1 ;;
esac

case "a" in
  [^abc]) printf '%s\n' "FAIL: [^abc] matched 'a'" >&2; exit 1 ;;
esac

case "d" in
  [!abc]) ;;
  *) printf '%s\n' "FAIL: [!abc] did not match 'd'" >&2; exit 1 ;;
esac

case "b" in
  [!abc]) printf '%s\n' "FAIL: [!abc] matched 'b'" >&2; exit 1 ;;
esac

exit 0
