# SHALL-09-03-05-010
# "A matching list expression specifies a list that shall match any single
#  character that is matched by one of the expressions represented in the list.
#  An ordinary character in the list shall only match that character"
# Verify matching list expressions match exactly the listed characters.

case "b" in
  [abc]) ;;
  *) printf '%s\n' "FAIL: [abc] did not match 'b'" >&2; exit 1 ;;
esac

case "d" in
  [abc]) printf '%s\n' "FAIL: [abc] matched 'd'" >&2; exit 1 ;;
  *) ;;
esac

case "a" in
  [abc]) ;;
  *) printf '%s\n' "FAIL: [abc] did not match 'a'" >&2; exit 1 ;;
esac

case "c" in
  [abc]) ;;
  *) printf '%s\n' "FAIL: [abc] did not match 'c'" >&2; exit 1 ;;
esac

exit 0
