# SHALL-09-03-05-003
# "A matching list expression specifies a list that shall match any single
#  character that is matched by one of the expressions represented in the list."
# Verify [abc] matches a, b, or c and nothing else.

_pass=true
for _c in a b c; do
  case "$_c" in
    [abc]) ;;
    *) _pass=false ;;
  esac
done

case "d" in
  [abc]) _pass=false ;;
esac

if [ "$_pass" != "true" ]; then
  printf '%s\n' "FAIL: [abc] matching list incorrect" >&2
  exit 1
fi

exit 0
